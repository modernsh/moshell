use lexer::token::TokenType;

use crate::ast::variable::{TypedVariable, VarDeclaration, VarKind};
use crate::ast::Expr;
use crate::moves::{of_type, of_types, space, spaces, MoveOperations};
use crate::parser::{ParseResult, Parser};

pub trait VarDeclarationParser<'a> {
    /// Parses a variable declaration.
    fn var_declaration(&mut self) -> ParseResult<Expr<'a>>;
}

impl<'a> VarDeclarationParser<'a> for Parser<'a> {
    /// Parses a variable declaration.
    fn var_declaration(&mut self) -> ParseResult<Expr<'a>> {
        let kind = match self.cursor.next()?.token_type {
            TokenType::Var => VarKind::Var,
            TokenType::Val => VarKind::Val,
            _ => return self.expected("expected var or val keywords"),
        };
        let name = self.cursor.force(
            space().and_then(of_type(TokenType::Identifier)),
            "Expected variable name.",
        )?;

        let ty = match self
            .cursor
            .advance(spaces().then(of_type(TokenType::Colon)))
        {
            None => None,
            Some(_) => Some(self.cursor.force(
                spaces().then(of_type(TokenType::Identifier)),
                "Expected variable type",
            )?),
        };
        let initializer = match self.cursor.advance(space().then(of_type(TokenType::Equal))) {
            None => {
                self.cursor.force(
                    of_types(&[TokenType::NewLine, TokenType::EndOfFile]),
                    "Expected newline after variable declaration",
                )?;
                None
            }
            
            Some(_) => Some(self.expression()?),
        };

        Ok(Expr::VarDeclaration(VarDeclaration {
            kind,
            var: TypedVariable { name, ty },
            initializer: initializer.map(Box::new),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::literal::{Literal, LiteralValue};
    use crate::ast::Expr;
    use crate::parser::Parser;
    use lexer::lexer::lex;
    use lexer::token::Token;
    use pretty_assertions::assert_eq;

    #[test]
    fn val_declaration() {
        let tokens = lex("val variable");
        let ast = Parser::new(tokens)
            .var_declaration()
            .expect("failed to parse");
        assert_eq!(
            ast,
            Expr::VarDeclaration(VarDeclaration {
                kind: VarKind::Val,
                var: TypedVariable {
                    name: Token::new(TokenType::Identifier, "variable"),
                    ty: None
                },
                initializer: None,
            })
        )
    }

    #[test]
    fn val_declaration_with_type() {
        let tokens = lex("val variable: Array");
        let ast = Parser::new(tokens)
            .var_declaration()
            .expect("failed to parse");
        assert_eq!(
            ast,
            Expr::VarDeclaration(VarDeclaration {
                kind: VarKind::Val,
                var: TypedVariable {
                    name: Token::new(TokenType::Identifier, "variable"),
                    ty: Some(Token::new(TokenType::Identifier, "Array")),
                },
                initializer: None,
            })
        )
    }

    #[test]
    fn val_declaration_with_type_no_colon() {
        let tokens = lex("val variable Array");
        Parser::new(tokens)
            .var_declaration()
            .expect_err("did not fail");
    }

    #[test]
    fn val_declaration_inferred() {
        let tokens = lex("val variable = 'hello $test'");
        let ast = Parser::new(tokens)
            .var_declaration()
            .expect("failed to parse");
        assert_eq!(
            ast,
            Expr::VarDeclaration(VarDeclaration {
                kind: VarKind::Val,
                var: TypedVariable {
                    name: Token::new(TokenType::Identifier, "variable"),
                    ty: None,
                },
                initializer: Some(Box::from(Expr::Literal(Literal {
                    token: Token::new(TokenType::Quote, "'"),
                    parsed: LiteralValue::String("hello $test".to_string()),
                }))),
            })
        )
    }
}
