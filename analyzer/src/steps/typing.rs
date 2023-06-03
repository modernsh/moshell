use crate::dependency::topological_sort;
use crate::diagnostic::{Diagnostic, DiagnosticID};
use crate::engine::Engine;
use crate::environment::Environment;
use crate::relations::{Relations, SourceObjectId};
use crate::types::ctx::TypeContext;
use crate::types::hir::{ExprKind, TypedExpr};
use crate::types::ty::Type;
use crate::types::{Typing, ERROR, FLOAT, INT, NOTHING, STRING};
use ast::function::FunctionParameter;
use ast::value::LiteralValue;
use ast::Expr;
use context::source::SourceSegmentHolder;
use std::collections::HashMap;

pub fn apply_types(
    engine: &Engine,
    relations: &Relations,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<SourceObjectId, TypedExpr> {
    let mut typing = Typing::lang();
    let mut ctx = TypeContext::lang();
    let environments = topological_sort(&relations.build_dependencies(engine));
    let mut typed = HashMap::new();
    for env_id in environments {
        typed.insert(
            env_id,
            apply_types_to_source(
                engine,
                relations,
                diagnostics,
                &mut typing,
                &mut ctx,
                env_id,
            ),
        );
    }
    typed
}

fn apply_types_to_source(
    engine: &Engine,
    relations: &Relations,
    diagnostics: &mut Vec<Diagnostic>,
    typing: &mut Typing,
    ctx: &mut TypeContext,
    source_id: SourceObjectId,
) -> TypedExpr {
    let expr = engine.get_expression(source_id).unwrap();
    let env = engine.get_environment(source_id).unwrap();
    ctx.prepare(source_id);
    ascribe_types(relations, diagnostics, typing, ctx, env, expr)
}

/// Ascribes types to the given expression.
///
/// In case of an error, the expression is still returned, but the type is set to [`ERROR`].
pub fn ascribe_types(
    relations: &Relations,
    diagnostics: &mut Vec<Diagnostic>,
    typing: &mut Typing,
    ctx: &mut TypeContext,
    env: &Environment,
    expr: &Expr,
) -> TypedExpr {
    match expr {
        Expr::Literal(lit) => {
            let ty = match lit.parsed {
                LiteralValue::Int(_) => INT,
                LiteralValue::Float(_) => FLOAT,
                LiteralValue::String(_) => STRING,
            };
            TypedExpr {
                kind: ExprKind::Literal(lit.parsed.clone()),
                ty,
                segment: lit.segment.clone(),
            }
        }
        Expr::VarDeclaration(decl) => {
            let initializer = decl
                .initializer
                .as_ref()
                .map(|expr| {
                    Box::new(ascribe_types(
                        relations,
                        diagnostics,
                        typing,
                        ctx,
                        env,
                        expr,
                    ))
                })
                .expect("Variables without initializers are not supported yet");
            ctx.push_local_type(initializer.ty);
            if let Some(type_annotation) = &decl.var.ty {
                let type_annotation = ctx.resolve(type_annotation).unwrap_or(ERROR);
                if type_annotation == ERROR {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticID::UnknownType,
                        ctx.source,
                        "Unknown type annotation",
                    ));
                } else if typing.unify(type_annotation, initializer.ty).is_err() {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticID::TypeMismatch,
                        ctx.source,
                        "Type mismatch",
                    ));
                }
            }
            TypedExpr {
                kind: ExprKind::Declare {
                    name: decl.var.name.to_owned(),
                    value: Some(initializer),
                },
                ty: NOTHING,
                segment: decl.segment.clone(),
            }
        }
        Expr::VarReference(var) => {
            let symbol = env.get_raw_symbol(var.segment.clone()).unwrap();
            let type_id = ctx.get(relations, symbol).unwrap();
            TypedExpr {
                kind: ExprKind::Reference {
                    name: var.name.to_owned(),
                },
                ty: type_id,
                segment: var.segment.clone(),
            }
        }
        Expr::Block(block) => {
            let expressions = block
                .expressions
                .iter()
                .map(|expr| ascribe_types(relations, diagnostics, typing, ctx, env, expr))
                .collect::<Vec<_>>();
            let ty = expressions.last().map(|expr| expr.ty).unwrap_or(NOTHING);
            TypedExpr {
                kind: ExprKind::Block(expressions),
                ty,
                segment: block.segment.clone(),
            }
        }
        Expr::FunctionDeclaration(fun) => {
            let type_id = typing.add_type(Type::Function {
                parameters: fun
                    .parameters
                    .iter()
                    .map(|param| match param {
                        FunctionParameter::Named(named) => named
                            .ty
                            .as_ref()
                            .map(|ty| ctx.resolve(ty).unwrap_or(ERROR))
                            .unwrap_or(STRING),
                        FunctionParameter::Variadic(_) => todo!("Arrays are not supported yet"),
                    })
                    .collect(),
                return_type: fun
                    .return_type
                    .as_ref()
                    .map(|ty| ctx.resolve(ty).unwrap_or(ERROR))
                    .unwrap_or(NOTHING),
            });
            ctx.push_local_type(type_id);
            TypedExpr {
                kind: ExprKind::Declare {
                    name: fun.name.to_owned(),
                    value: None,
                },
                ty: NOTHING,
                segment: fun.segment.clone(),
            }
        }
        Expr::Binary(bin) => {
            let left_expr = ascribe_types(relations, diagnostics, typing, ctx, env, &bin.left);
            let right_expr = ascribe_types(relations, diagnostics, typing, ctx, env, &bin.right);
            let ty = typing.unify(left_expr.ty, right_expr.ty).unwrap_or(ERROR);
            TypedExpr {
                kind: ExprKind::Binary {
                    lhs: Box::new(left_expr),
                    op: bin.op,
                    rhs: Box::new(right_expr),
                },
                ty,
                segment: bin.segment(),
            }
        }
        Expr::If(block) => {
            let condition =
                ascribe_types(relations, diagnostics, typing, ctx, env, &block.condition);
            let then = ascribe_types(
                relations,
                diagnostics,
                typing,
                ctx,
                env,
                &block.success_branch,
            );
            let otherwise = block.fail_branch.as_ref().map(|expr| {
                Box::new(ascribe_types(
                    relations,
                    diagnostics,
                    typing,
                    ctx,
                    env,
                    expr,
                ))
            });
            let ty = typing
                .unify(
                    then.ty,
                    otherwise.as_ref().map(|expr| expr.ty).unwrap_or(NOTHING),
                )
                .unwrap_or(ERROR);
            TypedExpr {
                kind: ExprKind::Conditional {
                    condition: Box::new(condition),
                    then: Box::new(then),
                    otherwise,
                },
                ty,
                segment: block.segment.clone(),
            }
        }
        Expr::Call(call) => {
            let args = call
                .arguments
                .iter()
                .map(|expr| ascribe_types(relations, diagnostics, typing, ctx, env, expr))
                .collect::<Vec<_>>();
            TypedExpr {
                kind: ExprKind::ProcessCall(args),
                ty: NOTHING,
                segment: call.segment(),
            }
        }
        Expr::ProgrammaticCall(call) => {
            let arguments = call
                .arguments
                .iter()
                .map(|expr| ascribe_types(relations, diagnostics, typing, ctx, env, expr))
                .collect::<Vec<_>>();
            let symbol = env.get_raw_symbol(call.segment.clone()).unwrap();
            let type_id = ctx.get(relations, symbol).unwrap();
            let return_type = match typing.get_type(type_id).unwrap() {
                Type::Function {
                    parameters,
                    return_type,
                } => {
                    if parameters.len() != arguments.len() {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticID::TypeMismatch,
                            ctx.source,
                            "Wrong number of arguments",
                        ));
                        ERROR
                    } else {
                        for (param, arg) in parameters.iter().zip(arguments.iter()) {
                            if typing.unify(*param, arg.ty).is_err() {
                                diagnostics.push(Diagnostic::new(
                                    DiagnosticID::TypeMismatch,
                                    ctx.source,
                                    "Type mismatch",
                                ));
                            }
                        }
                        return_type
                    }
                }
                _ => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticID::TypeMismatch,
                        ctx.source,
                        "Cannot invoke non function type",
                    ));
                    ERROR
                }
            };
            TypedExpr {
                kind: ExprKind::FunctionCall {
                    name: call.name.to_owned(),
                    arguments,
                },
                ty: return_type,
                segment: call.segment.clone(),
            }
        }
        _ => todo!("{expr:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::StaticImporter;
    use crate::name::Name;
    use crate::steps::collect::SymbolCollector;
    use crate::types::ty::Type;
    use context::source::Source;
    use parser::parse_trusted;

    pub(crate) fn extract_type(source: Source) -> Result<Type, Vec<Diagnostic>> {
        let mut engine = Engine::default();
        let mut relations = Relations::default();
        let typing = Typing::lang();
        let name = Name::new(source.name);
        let mut diagnostics = SymbolCollector::collect_symbols(
            &mut engine,
            &mut relations,
            name.clone(),
            &mut StaticImporter::new([(name, source)], parse_trusted),
        );
        assert_eq!(diagnostics, vec![]);
        let typed = apply_types(&mut engine, &mut relations, &mut diagnostics);
        let expr = typed.get(&SourceObjectId(0)).unwrap();
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(typing.get_type(expr.ty).unwrap())
    }

    #[test]
    fn single_literal() {
        let res = extract_type(Source::unknown("1"));
        assert_eq!(res, Ok(Type::Int));
    }

    #[test]
    fn correct_type_annotation() {
        let res = extract_type(Source::unknown("val a: Int = 1"));
        assert_eq!(res, Ok(Type::Nothing));
    }

    #[test]
    fn coerce_type_annotation() {
        let res = extract_type(Source::unknown("val a: Float = 4"));
        assert_eq!(res, Ok(Type::Nothing));
    }

    #[test]
    fn no_coerce_type_annotation() {
        let res = extract_type(Source::unknown("val a: Int = 1.6"));
        assert_eq!(
            res,
            Err(vec![Diagnostic::new(
                DiagnosticID::TypeMismatch,
                SourceObjectId(0),
                "Type mismatch",
            )])
        );
    }

    #[test]
    fn unknown_type_annotation() {
        let res = extract_type(Source::unknown("val a: ABC = 1.6"));
        assert_eq!(
            res,
            Err(vec![Diagnostic::new(
                DiagnosticID::UnknownType,
                SourceObjectId(0),
                "Unknown type annotation",
            )])
        );
    }

    #[test]
    fn condition_same_type() {
        let res = extract_type(Source::unknown("if true; 1; else 2"));
        assert_eq!(res, Ok(Type::Int));
    }

    #[test]
    fn function_return_type() {
        let res = extract_type(Source::unknown("fun one() -> Int = 1\none()"));
        assert_eq!(res, Ok(Type::Int));
    }

    #[test]
    fn wrong_arguments() {
        let res = extract_type(Source::unknown(
            "fun square(n: Int) = $(( $n * $n ))\nsquare(9, 9)",
        ));
        assert_eq!(
            res,
            Err(vec![Diagnostic::new(
                DiagnosticID::TypeMismatch,
                SourceObjectId(0),
                "Wrong number of arguments",
            )])
        );
    }

    #[test]
    fn wrong_arguments_type() {
        let res = extract_type(Source::unknown(
            "fun dup(str: String) -> String = $str\ndup(4)",
        ));
        assert_eq!(
            res,
            Err(vec![Diagnostic::new(
                DiagnosticID::TypeMismatch,
                SourceObjectId(0),
                "Type mismatch",
            )])
        );
    }

    #[test]
    fn cannot_invoke_non_function() {
        let res = extract_type(Source::unknown("val test = 1;test()"));
        assert_eq!(
            res,
            Err(vec![Diagnostic::new(
                DiagnosticID::TypeMismatch,
                SourceObjectId(0),
                "Cannot invoke non function type",
            )])
        );
    }
}
