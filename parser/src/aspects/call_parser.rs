use crate::ast::callable::Call;
use crate::ast::Expr;
use crate::moves::{eox, MoveOperations, spaces};
use crate::parser::{Parser, ParseResult};

///A parse aspect for command and function calls
pub trait CallParser<'a> {
    ///Attempts to parse next call expression
    fn call(&mut self) -> ParseResult<Expr<'a>>;
}

impl<'a> CallParser<'a> for Parser<'a> {
    fn call(&mut self) -> ParseResult<Expr<'a>> {
        let mut args = vec![self.expression()?];
        //End Of Expression \!(; + \n)
        while !self.cursor.is_at_end() && self.cursor.advance(spaces().then(eox())).is_none()
        {
            args.push(self.expression()?);
        }

        Ok(Expr::Call(Call { arguments: args }))
    }
}

#[cfg(test)]
mod tests {
    use lexer::lexer::lex;
    use lexer::token::{Token, TokenType};

    use crate::ast::callable::Call;
    use crate::ast::Expr;
    use crate::ast::literal::{Literal, LiteralValue};
    use crate::parse;
    use pretty_assertions::assert_eq;

    #[test]
    fn multiple_calls() {
        let tokens = lex("grep -E regex; echo test");
        let parsed = parse(tokens).expect("parsing error");
        assert_eq!(
            parsed,
            vec![
                Expr::Call(Call {
                    arguments: vec![
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "grep"),
                            parsed: LiteralValue::String("grep".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "E"),
                            parsed: LiteralValue::String("-E".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "regex"),
                            parsed: LiteralValue::String("regex".to_string()),
                        }),
                    ],
                }),
                Expr::Call(Call {
                    arguments: vec![
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "echo"),
                            parsed: LiteralValue::String("echo".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "test"),
                            parsed: LiteralValue::String("test".to_string()),
                        }),
                    ],
                }),
            ]
        )
    }

    #[test]
    fn escaped_call() {

        let tokens = lex("grep -E regex \\; echo test");
        let parsed = parse(tokens).expect("parsing error");
        assert_eq!(
            parsed,
            vec![
                Expr::Call(Call {
                    arguments: vec![
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "grep"),
                            parsed: LiteralValue::String("grep".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "E"),
                            parsed: LiteralValue::String("-E".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "regex"),
                            parsed: LiteralValue::String("regex".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::BackSlash, "\\"),
                            parsed: LiteralValue::String(";".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "echo"),
                            parsed: LiteralValue::String("echo".to_string()),
                        }),
                        Expr::Literal(Literal {
                            token: Token::new(TokenType::Identifier, "test"),
                            parsed: LiteralValue::String("test".to_string()),
                        }),
                    ],
                }),
            ]
        )
    }



}