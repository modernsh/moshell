use std::collections::HashMap;
use std::fmt;
use std::iter::once;

use ast::call::{MethodCall, ProgrammaticCall};
use ast::function::{FunctionDeclaration, FunctionParameter};
use ast::Expr;
use context::source::{SourceSegment, SourceSegmentHolder};

use crate::diagnostic::{Diagnostic, DiagnosticID, Observation, SourceLocation};
use crate::reef::ReefId;
use crate::relations::{Definition, LocalId, SourceId, SymbolRef};
use crate::steps::typing::bounds::TypesBounds;
use crate::steps::typing::coercion::{
    convert_description, convert_expression, convert_many, resolve_type_annotation,
};
use crate::steps::typing::exploration::{Exploration, Links};
use crate::steps::typing::{ascribe_types, ExpressionValue, TypingState};
use crate::types::engine::{Chunk, CodeEntry};
use crate::types::hir::{ExprKind, TypedExpr};
use crate::types::ty::{FunctionType, MethodType, Parameter, Type, TypeRef};
use crate::types::{ERROR, STRING, UNIT};

/// An identified return during the exploration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Return {
    /// The returned type.
    pub(super) ty: TypeRef,

    /// The segment where the return is located.
    pub(super) segment: SourceSegment,
}

/// Identifies a function that correspond to a call.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FunctionMatch {
    pub(super) type_arguments: Vec<TypeRef>,
    /// The converted arguments to pass to the function.
    ///
    /// If any conversion is required, it will be done here.
    pub(super) arguments: Vec<TypedExpr>,

    /// The function identifier to call.
    pub(super) definition: Definition,

    /// The function return type.
    pub(super) return_type: TypeRef,

    /// The function's reef
    pub(super) reef: ReefId,
}

/// Gets the returned type of a function.
///
/// This verifies the type annotation if present against all the return types,
/// or try to guess the return type.
pub(super) fn infer_return(
    func: &FunctionDeclaration,
    expected_return_type: TypeRef,
    links: Links,
    typed_func_body: Option<&TypedExpr>,
    diagnostics: &mut Vec<Diagnostic>,
    exploration: &mut Exploration,
) -> TypeRef {
    if let Some(typed_func_body) = typed_func_body {
        let last = get_last_segment(typed_func_body);
        // If the last statement is a return, we don't need re-add it
        if exploration
            .returns
            .last()
            .map_or(true, |ret| ret.segment != last.segment)
            && last.ty.is_something()
            && last.ty.is_ok()
        {
            exploration.returns.push(Return {
                ty: typed_func_body.ty,
                segment: last.segment.clone(),
            });
        }
    }

    let mut typed_return_locations: Vec<_> = Vec::new();

    for ret in &exploration.returns {
        if convert_description(
            exploration,
            expected_return_type,
            ret.ty,
            &mut TypesBounds::inactive(),
            true,
        )
        .is_err()
        {
            typed_return_locations.push(Observation::here(
                links.source,
                exploration.externals.current,
                ret.segment.clone(),
                if func.return_type.is_some() {
                    format!(
                        "Found `{}`",
                        exploration.new_type_view(ret.ty, &TypesBounds::inactive())
                    )
                } else {
                    format!(
                        "Returning `{}`",
                        exploration.new_type_view(ret.ty, &TypesBounds::inactive())
                    )
                },
            ));
        }
    }

    if typed_return_locations.is_empty() {
        return expected_return_type;
    }

    if let Some(return_type_annotation) = func.return_type.as_ref() {
        diagnostics.push(
            Diagnostic::new(DiagnosticID::TypeMismatch, "Type mismatch")
                .with_observations(typed_return_locations)
                .with_observation(Observation::context(
                    links.source,
                    exploration.externals.current,
                    return_type_annotation.segment(),
                    format!(
                        "Expected `{}` because of return type",
                        exploration.new_type_view(expected_return_type, &TypesBounds::inactive()),
                    ),
                )),
        );
        return ERROR;
    }

    let Some(body) = &func.body else {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::CannotInfer,
                "Function declaration needs explicit return type",
            )
            .with_observations(typed_return_locations)
            .with_help("Explicit the function's return type as it's not defined."),
        );

        return ERROR;
    };

    if matches!(body.as_ref(), Expr::Block(_)) {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::CannotInfer,
                "Return type is not inferred for block functions",
            )
            .with_observations(typed_return_locations)
            .with_help("Try adding an explicit return type to the function"),
        );

        return ERROR;
    }
    let segment = func.segment().start..body.segment().start;
    let types: Vec<_> = exploration.returns.iter().map(|ret| ret.ty).collect();
    let unify = convert_many(exploration, &mut TypesBounds::inactive(), types);

    if let Ok(common_type) = unify {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::CannotInfer,
                "Return type inference is not supported yet",
            )
            .with_observation(Observation::context(
                links.source,
                exploration.externals.current,
                segment,
                "No return type is specified",
            ))
            .with_observations(typed_return_locations)
            .with_help(format!(
                "Add -> {} to the function declaration",
                exploration.new_type_view(common_type, &TypesBounds::inactive()),
            )),
        );
    } else {
        diagnostics.push(
            Diagnostic::new(DiagnosticID::CannotInfer, "Failed to infer return type")
                .with_observation(Observation::context(
                    links.source,
                    exploration.externals.current,
                    segment,
                    "This function returns multiple types",
                ))
                .with_observations(typed_return_locations)
                .with_help("Try adding an explicit return type to the function"),
        );
    }
    ERROR
}

fn apply_bounds(exploration: &mut Exploration, ty: TypeRef, bounds: &TypesBounds) -> TypeRef {
    let ty_ref = bounds.get_bound(ty);
    let ty = exploration.get_type(ty_ref).unwrap();
    if let Type::Instantiated(base, params) = ty {
        let base = bounds.get_bound(*base);
        let params: Vec<_> = params
            .clone()
            .into_iter()
            .map(|ty| apply_bounds(exploration, ty, bounds))
            .collect();

        let type_id = exploration
            .typing
            .add_type(Type::Instantiated(base, params), None);
        return TypeRef::new(exploration.externals.current, type_id);
    }

    ty_ref
}

/// Ensures that the return type does not contains any reference to given type parameters of function.
fn check_for_leaked_type_parameters(
    exploration: &Exploration,
    types_parameters: &[TypeRef],
    return_type: TypeRef,
    source: SourceId,
    call_segment: SourceSegment,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypeRef {
    let mut leaked_types = Vec::new();

    fn collect_leaked_types(
        exploration: &Exploration,
        not_to_leak: &[TypeRef],
        tpe: TypeRef,
        leaked_types: &mut Vec<TypeRef>,
    ) {
        if not_to_leak.contains(&tpe) {
            leaked_types.push(tpe)
        }
        let ty = exploration.get_type(tpe).unwrap();
        if let Type::Instantiated(base, params) = ty {
            collect_leaked_types(exploration, not_to_leak, *base, leaked_types);
            for param in params {
                collect_leaked_types(exploration, not_to_leak, *param, leaked_types);
            }
        }
    }

    collect_leaked_types(
        exploration,
        types_parameters,
        return_type,
        &mut leaked_types,
    );

    if let Some((first, tail)) = leaked_types.split_first() {
        let leaked_types_str = {
            tail.iter().fold(
                format!(
                    "`{}`",
                    exploration.new_type_view(*first, &TypesBounds::inactive())
                ),
                |acc, it| {
                    format!(
                        "{acc}, `{}`",
                        exploration.new_type_view(*it, &TypesBounds::inactive())
                    )
                },
            )
        };

        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::CannotInfer,
                "Cannot infer parameter types of function",
            )
            .with_observation(Observation::here(
                source,
                exploration.externals.current,
                call_segment,
                format!("please provide explicit types for generic parameters {leaked_types_str}"),
            )),
        );
        ERROR
    } else {
        return_type
    }
}

/// create a basic chunk from a function declaration
/// type its parameters, type parameters and return type
pub(super) fn type_function_signature(
    func: &FunctionDeclaration,
    exploration: &mut Exploration,
    function_links: Links,
    diagnostics: &mut Vec<Diagnostic>,
) -> Chunk {
    let mut type_params = Vec::new();
    let mut params = Vec::new();

    let func_source = function_links.source;

    for type_param in &func.type_parameters {
        let param_type_id = exploration
            .typing
            .add_type(Type::Polytype, Some(type_param.name.to_string()));
        let param_type_ref = TypeRef::new(exploration.externals.current, param_type_id);
        type_params.push(param_type_ref);
        exploration
            .ctx
            .push_local_typed(func_source, param_type_ref);
        exploration
            .ctx
            .bind_name(type_param.name.to_string(), param_type_id);
    }

    let tparam_count = func.type_parameters.len();
    for (param_offset, param) in func.parameters.iter().enumerate() {
        let param = type_parameter(
            LocalId(tparam_count + param_offset),
            exploration,
            param,
            function_links,
            diagnostics,
        );
        exploration.ctx.push_local_typed(func_source, param.ty);
        params.push(param);
    }

    let return_type = func.return_type.as_ref().map_or(UNIT, |ty| {
        resolve_type_annotation(exploration, function_links, ty, diagnostics)
    });

    let type_id = exploration.typing.add_type(
        Type::Function(Definition::User(func_source)),
        Some(func.name.to_string()),
    );
    let type_ref = TypeRef::new(exploration.externals.current, type_id);

    Chunk {
        expression: Some(TypedExpr {
            kind: ExprKind::Noop,
            ty: type_ref,
            segment: func.segment(),
        }),
        type_parameters: type_params,
        parameters: params,
        return_type,
    }
}

/// Checks the type of a call expression.
pub(super) fn type_call(
    call: &ProgrammaticCall,
    exploration: &mut Exploration,
    links: Links,
    state: TypingState,
    diagnostics: &mut Vec<Diagnostic>,
) -> FunctionMatch {
    let arguments = &call.arguments;

    let call_symbol_ref = links.env().get_raw_symbol(call.segment()).unwrap();

    let (fun_reef, fun_source) = match call_symbol_ref {
        SymbolRef::Local(_) => (exploration.externals.current, links.source),
        SymbolRef::External(r) => {
            let call_symbol = links.relations[r].state.expect_resolved("unresolved");
            (call_symbol.reef, call_symbol.source)
        }
    };

    let function_type_ref = exploration
        .get_var(fun_source, call_symbol_ref, links.relations)
        .unwrap()
        .type_ref;

    match exploration.get_type(function_type_ref).unwrap() {
        &Type::Function(definition) => {
            let entry: CodeEntry = exploration.get_entry(fun_reef, definition).unwrap();
            let parameters = entry.parameters().to_owned(); // TODO: avoid clone
            let return_type = entry.return_type();
            if parameters.len() != arguments.len() {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticID::TypeMismatch,
                        format!(
                            "This function takes {} {} but {} {} supplied",
                            parameters.len(),
                            pluralize(parameters.len(), "argument", "arguments"),
                            arguments.len(),
                            pluralize(arguments.len(), "was", "were"),
                        ),
                    )
                    .with_observation(Observation::here(
                        links.source,
                        exploration.externals.current,
                        call.segment.clone(),
                        "Function is called here",
                    )),
                );

                let type_arguments = entry.type_parameters().to_vec();

                let arguments = arguments
                    .iter()
                    .map(|expr| ascribe_types(exploration, links, diagnostics, expr, state))
                    .collect::<Vec<_>>();

                FunctionMatch {
                    type_arguments,
                    arguments,
                    definition: Definition::error(),
                    return_type: ERROR,
                    reef: fun_reef,
                }
            } else {
                let type_arguments = entry.type_parameters().to_vec();

                let expected_type = if let ExpressionValue::Expected(t) = state.local_value {
                    Some(t)
                } else {
                    None
                };

                let mut bounds = build_bounds(
                    &call.type_parameters,
                    fun_reef,
                    definition,
                    expected_type,
                    exploration,
                    links,
                    diagnostics,
                );

                let mut casted_arguments = Vec::with_capacity(parameters.len());
                for (param, arg) in parameters.iter().cloned().zip(arguments) {
                    let param_bound = bounds.get_bound(param.ty);

                    let arg = ascribe_types(
                        exploration,
                        links,
                        diagnostics,
                        arg,
                        state.with_local_value(ExpressionValue::Expected(param_bound)),
                    );

                    let casted_argument = convert_expression(
                        arg,
                        param_bound,
                        &mut bounds,
                        exploration,
                        links.source,
                        diagnostics,
                    );

                    let casted_argument = match casted_argument {
                        Ok(arg) => {
                            bounds.update_bounds(param.ty, arg.ty, exploration);
                            arg
                        }
                        Err(arg) => {
                            diagnostics.push(diagnose_arg_mismatch(
                                exploration,
                                links.source,
                                exploration.externals.current,
                                fun_reef,
                                &param,
                                &arg,
                                &bounds,
                            ));
                            arg
                        }
                    };

                    casted_arguments.push(casted_argument);
                }

                let return_type = apply_bounds(exploration, return_type, &bounds);

                let return_type = check_for_leaked_type_parameters(
                    exploration,
                    &type_arguments,
                    return_type,
                    links.source,
                    call.segment(),
                    diagnostics,
                );

                FunctionMatch {
                    type_arguments,
                    arguments: casted_arguments,
                    definition,
                    return_type,
                    reef: fun_reef,
                }
            }
        }
        _ => {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticID::TypeMismatch,
                    "Cannot invoke non function type",
                )
                .with_observation(Observation::here(
                    links.source,
                    exploration.externals.current,
                    call.segment(),
                    format!(
                        "Call expression requires function, found `{}`",
                        exploration.new_type_view(function_type_ref, &TypesBounds::inactive())
                    ),
                )),
            );

            let arguments = arguments
                .iter()
                .map(|expr| ascribe_types(exploration, links, diagnostics, expr, state))
                .collect::<Vec<_>>();

            FunctionMatch {
                type_arguments: Vec::new(),
                arguments,
                definition: Definition::error(),
                return_type: ERROR,
                reef: fun_reef,
            }
        }
    }
}

/// update given bounds to update type parameters bounds of the function's return type from the given hint
fn infer_return_from_hint(
    exploration: &Exploration,
    return_type: TypeRef,
    return_type_hint: TypeRef,
    bounds: &mut HashMap<TypeRef, TypeRef>,
) {
    let return_tpe = exploration.get_type(return_type).unwrap();
    let hint_tpe = exploration.get_type(return_type_hint).unwrap();
    match (return_tpe, hint_tpe) {
        (Type::Polytype, _) => {
            bounds.insert(return_type, return_type_hint);
        }
        (Type::Instantiated(_, return_params), Type::Instantiated(_, hint_params)) => {
            for (return_param, hint_param) in return_params.iter().zip(hint_params) {
                infer_return_from_hint(exploration, *return_param, *hint_param, bounds)
            }
        }
        _ => {}
    }
}

fn extract_polytypes(tpe_ref: TypeRef, exploration: &Exploration) -> Vec<TypeRef> {
    let tpe = exploration.get_type(tpe_ref).unwrap();
    match tpe {
        Type::Polytype => once(tpe_ref).collect(),
        Type::Instantiated(base, params) => extract_polytypes(*base, exploration)
            .into_iter()
            .chain(
                params
                    .iter()
                    .flat_map(|ty| extract_polytypes(*ty, exploration)),
            )
            .collect(),
        _ => Vec::new(),
    }
}

/// search if given type is contained in given polytypes or has any type parameter contained in this list.
fn type_depends_of(tpe: TypeRef, polytypes: &Vec<TypeRef>, exploration: &Exploration) -> bool {
    if polytypes.contains(&tpe) {
        return true;
    }

    if let Type::Instantiated(base, params) = exploration.get_type(tpe).unwrap() {
        return type_depends_of(*base, polytypes, exploration)
            || params
                .iter()
                .any(|ty| type_depends_of(*ty, polytypes, exploration));
    }
    false
}

/// build type parameters bounds of a user-defined function.
/// The return hint is only applied if the function's return type does not depend on function's parameters.
fn build_bounds(
    user_bounds: &[ast::r#type::Type],
    fun_reef: ReefId,
    definition: Definition,
    return_hint: Option<TypeRef>,
    exploration: &mut Exploration,
    links: Links,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypesBounds {
    let user_bounds_types: Vec<_> = user_bounds
        .iter()
        .map(|ty| resolve_type_annotation(exploration, links, ty, diagnostics))
        .collect();

    let entry = exploration.get_entry(fun_reef, definition).unwrap();

    let entry_type_parameters = entry.type_parameters();

    let mut bounds = HashMap::new();

    // collect the functions' type parameters used in the parameters.
    let parameters_polytypes = entry
        .parameters()
        .iter()
        .flat_map(|p| extract_polytypes(p.ty, exploration))
        .collect();

    // Use the return type hint only if it does not contains a polytype bound with the parameters
    if !type_depends_of(entry.return_type(), &parameters_polytypes, exploration) {
        if let Some(hint) = return_hint {
            infer_return_from_hint(exploration, entry.return_type(), hint, &mut bounds);
        }
    }

    for (idx, type_param) in entry_type_parameters.iter().enumerate() {
        let user_bound = user_bounds_types.get(idx).cloned();

        // user has explicitly set a type bound
        if let Some(user_bound) = user_bound {
            bounds.insert(*type_param, user_bound);
        } else {
            // user expects an inference
            // if bounds is already know thanks to the given return type hint correlation with function types parameters
            // let it as is, else, bound the type param with itself
            if !bounds.contains_key(type_param) {
                bounds.insert(*type_param, *type_param);
            }
        }
    }

    if !user_bounds.is_empty() && user_bounds.len() != entry_type_parameters.len() {
        let first = user_bounds.first().unwrap();
        let last = user_bounds.last().unwrap();

        let segment = first.segment().start..last.segment().end;

        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::InvalidTypeArguments,
                "Wrong type argument count",
            )
            .with_observation(Observation::here(
                links.source,
                exploration.externals.current,
                segment,
                format!(
                    "`{}` type parameter specified, expected `{}`.",
                    user_bounds.len(),
                    entry_type_parameters.len()
                ),
            )),
        )
    }

    TypesBounds::new(bounds)
}

/// Checks the type of a method expression.
pub(super) fn find_operand_implementation(
    exploration: &Exploration,
    reef: ReefId,
    methods: &[MethodType],
    left: TypeRef,
    right: TypedExpr,
) -> Option<FunctionMatch> {
    for method in methods {
        if let [param] = &method.parameters.as_slice() {
            if param.ty == right.ty {
                let return_type = exploration.concretize(method.return_type, left);
                return Some(FunctionMatch {
                    type_arguments: vec![method.return_type],
                    arguments: vec![right],
                    definition: method.definition,
                    return_type,
                    reef,
                });
            }
        }
    }
    None
}

/// Checks the type of a method expression.
pub(super) fn type_method(
    method_call: &MethodCall,
    callee: &TypedExpr,
    links: Links,
    arguments: Vec<TypedExpr>,
    diagnostics: &mut Vec<Diagnostic>,
    exploration: &mut Exploration,
    source: SourceId,
) -> Option<FunctionMatch> {
    if callee.ty.is_err() {
        return None;
    }

    let type_args: Vec<_> = method_call
        .type_parameters
        .iter()
        .map(|t| resolve_type_annotation(exploration, links, t, diagnostics))
        .collect();

    let current_reef = exploration.externals.current;

    // Directly callable types just have a single method called `apply`
    let method_name = method_call.name.unwrap_or("apply");
    let type_methods = exploration.get_methods(callee.ty, method_name);
    if type_methods.is_none() {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::UnknownMethod,
                if method_call.name.is_some() {
                    format!(
                        "No method named `{method_name}` found for type `{}`",
                        exploration.new_type_view(callee.ty, &TypesBounds::inactive())
                    )
                } else {
                    format!(
                        "Type `{}` is not directly callable",
                        exploration.new_type_view(callee.ty, &TypesBounds::inactive())
                    )
                },
            )
            .with_observation((source, current_reef, method_call.segment.clone()).into()),
        );
        return None;
    }

    let methods = type_methods.unwrap(); // We just checked for None

    let result = find_exact_method(exploration, callee.ty, methods, &arguments, &type_args);
    if let Some((method, bounds)) = result {
        let type_parameters = method.type_parameters.clone();
        let definition = method.definition;

        let return_type = exploration.concretize(method.return_type, callee.ty);
        let return_type = apply_bounds(exploration, return_type, &bounds);
        let return_type = check_for_leaked_type_parameters(
            exploration,
            &type_parameters,
            return_type,
            links.source,
            method_call.segment(),
            diagnostics,
        );

        // We have an exact match
        return Some(FunctionMatch {
            type_arguments: type_parameters
                .iter()
                .map(|k| bounds.get_bound(*k))
                .collect(),
            arguments,
            definition,
            return_type,
            reef: callee.ty.reef,
        });
    }

    if methods.len() == 1 {
        // If there is only one method, we can give a more specific error by adding
        // an observation for each invalid type
        let method = methods.first().unwrap();

        if method.parameters.len() != arguments.len() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticID::TypeMismatch,
                    format!(
                        "This method takes {} {} but {} {} supplied",
                        method.parameters.len(),
                        pluralize(method.parameters.len(), "argument", "arguments"),
                        arguments.len(),
                        pluralize(arguments.len(), "was", "were")
                    ),
                )
                .with_observation(Observation::here(
                    source,
                    current_reef,
                    method_call.segment(),
                    "Method is called here",
                ))
                .with_help(format!(
                    "The method signature is `{}::{}`",
                    exploration.new_type_view(callee.ty, &TypesBounds::inactive()),
                    Signature::new(exploration, method_name, method)
                )),
            );
        } else {
            // cannot use `build_bounds` as it would imply to retrieve a native chunk from its definition in TypedEngine, which is
            // completely broken currently
            let mut bounds = TypesBounds::new(
                method
                    .type_parameters
                    .iter()
                    .map(|p| (*p, exploration.concretize(*p, callee.ty)))
                    .collect(),
            );

            for (param, arg) in method.parameters.iter().zip(arguments.iter()) {
                let param_bound = bounds.get_bound(param.ty);

                match convert_description(exploration, param_bound, arg.ty, &mut bounds, true) {
                    Ok(ty) => {
                        bounds.update_bounds(param.ty, ty, exploration);
                    }
                    Err(_) => {
                        let param = Parameter {
                            location: param.location.clone(),
                            ty: param_bound,
                            local_id: param.local_id,
                        };
                        let diagnostic = diagnose_arg_mismatch(
                            exploration,
                            source,
                            current_reef,
                            callee.ty.reef,
                            &param,
                            arg,
                            &bounds,
                        )
                        .with_observation(Observation::here(
                            source,
                            current_reef,
                            method_call.segment(),
                            "Arguments to this method are incorrect",
                        ));
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }
    } else {
        // If there are multiple methods, list them all
        diagnostics.push(
            Diagnostic::new(
                DiagnosticID::UnknownMethod,
                format!(
                    "No matching method found for `{method_name}::{}`",
                    exploration.new_type_view(callee.ty, &TypesBounds::inactive())
                ),
            )
            .with_observation(Observation::here(
                source,
                current_reef,
                method_call.segment(),
                "Method is called here",
            )),
        );
    }
    None
}

/// Generates a type mismatch between a parameter and an argument.
fn diagnose_arg_mismatch(
    exploration: &Exploration,
    source: SourceId,
    current_reef: ReefId,
    param_reef: ReefId,
    param: &Parameter,
    arg: &TypedExpr,
    bounds: &TypesBounds,
) -> Diagnostic {
    let diagnostic = Diagnostic::new(DiagnosticID::TypeMismatch, "Type mismatch").with_observation(
        Observation::here(
            source,
            current_reef,
            arg.segment.clone(),
            format!(
                "Expected `{}`, found `{}`",
                exploration.new_type_view(param.ty, bounds),
                exploration.new_type_view(arg.ty, bounds)
            ),
        ),
    );
    if let Some(location) = &param.location {
        diagnostic.with_observation(Observation::context(
            location.source,
            param_reef,
            location.segment.clone(),
            "Parameter is declared here",
        ))
    } else {
        diagnostic
    }
}

/// Find a matching method for the given arguments.
fn find_exact_method<'a>(
    exploration: &Exploration,
    obj: TypeRef,
    methods: &'a [MethodType],
    args: &[TypedExpr],
    type_args: &[TypeRef],
) -> Option<(&'a MethodType, TypesBounds)> {
    let bounds_base: HashMap<TypeRef, TypeRef> = type_args.iter().map(|p| (*p, *p)).collect();

    'methods: for method in methods {
        if method.parameters.len() != args.len() {
            continue;
        }

        let mut bounds = TypesBounds::new(bounds_base.clone());

        for (param, arg) in method.parameters.iter().zip(args.iter()) {
            let param_ty = exploration.concretize(param.ty, obj);
            let param_bound = bounds.get_bound(param_ty);

            let converted =
                convert_description(exploration, param_bound, arg.ty, &mut bounds, true);
            match converted {
                Ok(ty) => {
                    bounds.update_bounds(param.ty, ty, exploration);
                }
                Err(_) => continue 'methods,
            }
        }
        return Some((method, bounds));
    }
    None
}

/// Type check a single function parameter.
pub(super) fn type_parameter(
    local_id: LocalId,
    exploration: &mut Exploration,
    param: &FunctionParameter,
    links: Links,
    diagnostics: &mut Vec<Diagnostic>,
) -> Parameter {
    match param {
        FunctionParameter::Named(named) => {
            let type_id = named.ty.as_ref().map_or(STRING, |ty| {
                resolve_type_annotation(exploration, links, ty, diagnostics)
            });
            Parameter {
                location: Some(SourceLocation::new(
                    links.source,
                    exploration.externals.current,
                    named.segment.clone(),
                )),
                ty: type_id,
                local_id,
            }
        }
        FunctionParameter::Slf(_) => todo!("method not supported yet"),
        FunctionParameter::Variadic(_, _) => todo!("Arrays are not supported yet"),
    }
}

fn get_last_segment(expr: &TypedExpr) -> &TypedExpr {
    match &expr.kind {
        ExprKind::Block(expressions) => expressions.last().map_or(expr, get_last_segment),
        _ => expr,
    }
}

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

/// A formatted signature of a function.
struct Signature<'a> {
    exploration: &'a Exploration<'a>,
    name: &'a str,
    function: &'a FunctionType,
}

impl<'a> Signature<'a> {
    /// Creates a new signature.
    fn new(exploration: &'a Exploration<'a>, name: &'a str, function: &'a FunctionType) -> Self {
        Self {
            exploration,
            name,
            function,
        }
    }
}

impl fmt::Display for Signature<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;
        if let Some((first, parameters)) = self.function.parameters.split_first() {
            write!(
                f,
                "{}",
                self.exploration
                    .new_type_view(first.ty, &TypesBounds::inactive())
            )?;
            for param in parameters {
                write!(
                    f,
                    ", {}",
                    self.exploration
                        .new_type_view(param.ty, &TypesBounds::inactive())
                )?;
            }
        }
        if self.function.return_type.is_nothing() {
            write!(f, ")")
        } else {
            write!(
                f,
                ") -> {}",
                self.exploration
                    .new_type_view(self.function.return_type, &TypesBounds::inactive())
            )
        }
    }
}
