use crate::aspects::expr_list::ExpressionListAspect;
use crate::aspects::var_declaration::VarDeclarationAspect;
use crate::err::ParseErrorKind::Expected;
use crate::moves::{blanks, of_type, MoveOperations};
use crate::parser::{ParseResult, Parser};
use ast::lambda::LambdaDef;
use lexer::token::TokenType::{FatArrow, RoundedLeftBracket, RoundedRightBracket};

///Parse a lambda definition
pub trait LambdaDefinitionAspect<'a> {
    ///Parse a lambda definition (ex: (a) => $b + $a)
    fn parse_lambda_definition(&mut self) -> ParseResult<LambdaDef<'a>>;
}

impl<'a> LambdaDefinitionAspect<'a> for Parser<'a> {
    fn parse_lambda_definition(&mut self) -> ParseResult<LambdaDef<'a>> {
        let args = self.parse_implicit_list(
            RoundedLeftBracket,
            RoundedRightBracket,
            Self::parse_typed_var,
        )?;
        self.cursor.force_with(
            blanks().then(of_type(FatArrow)),
            "expected lambda arrow",
            Expected("=>".to_string()),
        )?;
        let body = Box::new(self.value()?);
        Ok(LambdaDef { args, body })
    }
}

#[cfg(test)]
mod tests {
    use crate::aspects::lambda_def::LambdaDefinitionAspect;
    use crate::err::{find_in, ParseError};
    use crate::err::ParseErrorKind::Unexpected;
    use crate::parser::Parser;
    use ast::call::Call;
    use ast::group::Block;
    use ast::lambda::LambdaDef;
    use ast::operation::{BinaryOperation, BinaryOperator};
    use ast::r#type::{SimpleType, Type};
    use ast::variable::{TypedVariable, VarReference};
    use ast::Expr;
    use context::source::Source;
    use pretty_assertions::assert_eq;

    #[test]
    fn simple_lambda_definition() {
        let source = Source::unknown("(a, b: Int) => $a + $b");
        let parsed = Parser::new(source.clone())
            .parse_lambda_definition()
            .expect("Failed to parse.");
        assert_eq!(
            parsed,
            LambdaDef {
                args: vec![
                    TypedVariable {
                        name: "a",
                        ty: None,
                        segment: Default::default(),
                    },
                    TypedVariable {
                        name: "b",
                        ty: Some(Type::Simple(SimpleType {
                            name: "Int",
                            params: Vec::new(),
                        })),
                        segment: Default::default(),
                    },
                ],
                body: Box::new(Expr::Binary(BinaryOperation {
                    left: Box::new(Expr::VarReference(VarReference {
                        name: "a",
                        segment: find_in(source.source, "$a"),
                    })),
                    op: BinaryOperator::Plus,
                    right: Box::new(Expr::VarReference(VarReference { name: "b", segment: find_in(source.source, "$b") })),
                })),
            }
        );
    }

    #[test]
    fn simple_lambda_definition_one_arg() {
        let source = Source::unknown("a => $a + $b");
        let parsed = Parser::new(source)
            .parse_lambda_definition()
            .expect("Failed to parse.");
        assert_eq!(
            parsed,
            LambdaDef {
                args: vec![TypedVariable {
                    name: "a",
                    ty: None,
                },],
                body: Box::new(Expr::Binary(BinaryOperation {
                    left: Box::new(Expr::VarReference(VarReference { name: "a" })),
                    op: BinaryOperator::Plus,
                    right: Box::new(Expr::VarReference(VarReference { name: "b" })),
                })),
            }
        );
    }

    #[test]
    fn simple_lambda_definition_one_arg_typed_wrapped() {
        let src = "(a: Int) => $a + $b";
        let source = Source::unknown(src);
        let parsed = Parser::new(source)
            .parse_lambda_definition()
            .expect("parse fail");
        assert_eq!(
            parsed,
            LambdaDef {
                args: vec![TypedVariable {
                    name: "a",
                    ty: Some(Type::Simple(SimpleType {
                        name: "Int",
                        params: Vec::new(),
                    })),
                },],
                body: Box::new(Expr::Binary(BinaryOperation {
                    left: Box::new(Expr::VarReference(VarReference { name: "a" })),
                    op: BinaryOperator::Plus,
                    right: Box::new(Expr::VarReference(VarReference { name: "b" })),
                })),
            }
        );
    }

    #[test]
    fn simple_lambda_definition_emptyargs() {
        let source = Source::unknown("() => {echo hey}");
        let parsed = Parser::new(source)
            .parse_lambda_definition()
            .expect("Failed to parse.");
        assert_eq!(
            parsed,
            LambdaDef {
                args: Vec::new(),
                body: Box::new(Expr::Block(Block {
                    expressions: vec![Expr::Call(Call {
                        arguments: vec![Expr::Literal("echo".into()), Expr::Literal("hey".into()),],
                        type_parameters: Vec::new(),
                    })]
                })),
            }
        );
    }

    #[test]
    fn simple_lambda_definition_noargs() {
        let source = Source::unknown(" => {echo hey}");
        let parsed = Parser::new(source)
            .parse_lambda_definition()
            .expect_err("parser did not fail");
        assert_eq!(
            parsed,
            ParseError {
                message: "Expected name.".to_string(),
                position: 1..3,
                kind: Unexpected,
            }
        );
    }
}
