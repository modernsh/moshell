use std::num::IntErrorKind;

use lexer::token::TokenType;

use crate::ast::*;
use crate::ast::literal::{Literal, LiteralValue};
use crate::parser::{Parser, ParseResult};

pub(crate) trait LiteralParser<'a> {
    fn literal(&mut self) -> ParseResult<Expr<'a>>;
    fn string_literal(&mut self) -> ParseResult<Expr<'a>>;
    fn parse_literal(&mut self) -> ParseResult<LiteralValue>;
}

impl<'a> LiteralParser<'a> for Parser<'a> {
    fn literal(&mut self) -> ParseResult<Expr<'a>> {
        Ok(Expr::Literal(Literal {
            token: self.cursor().peek_token(),
            parsed: self.parse_literal()?,
        }))
    }

    fn string_literal(&mut self) -> ParseResult<Expr<'a>> {
        let cursor = self.cursor();
        let token = cursor.next_token()?;
        let mut value = String::new();
        loop {
            if cursor.is_at_end() {
                return Err(cursor.mk_parse_error("Unterminated string literal."));
            }
            if cursor.meet_token(TokenType::Quote) {
                break;
            }
            value.push_str(cursor.next_token()?.value);
        }
        Ok(Expr::Literal(Literal {
            token,
            parsed: LiteralValue::String(value),
        }))
    }

    fn parse_literal(&mut self) -> ParseResult<LiteralValue> {
        let cursor = self.cursor();

        let token = cursor.next_token()?;
        match token.token_type {
            TokenType::IntLiteral => Ok(LiteralValue::Int(token.value.parse::<i64>().map_err(
                |e| match e.kind() {
                    IntErrorKind::PosOverflow | IntErrorKind::NegOverflow => {
                        self.cursor().mk_parse_error("Integer constant is too large.")
                    }
                    _ => self.cursor().mk_parse_error(e.to_string()),
                },
            )?)),
            TokenType::FloatLiteral => Ok(LiteralValue::Float(
                token
                    .value
                    .parse::<f64>()
                    .map_err(|e| cursor.mk_parse_error(e.to_string()))?,
            )),
            _ => Err(cursor.mk_parse_error("Expected a literal.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use lexer::token::Token;

    use crate::parse;
    use crate::parser::ParseError;

    use super::*;

    #[test]
    fn int_overflow() {
        let tokens = vec![Token::new(
            TokenType::IntLiteral,
            "123456789012345678901234567890",
        )];
        let parsed = parse(tokens);
        assert_eq!(
            parsed,
            Err(ParseError {
                message: "Integer constant is too large.".to_string(),
            })
        );
    }

    #[test]
    fn missing_quote() {
        let tokens = vec![Token::new(TokenType::Quote, "'")];
        let parsed = parse(tokens);
        assert_eq!(
            parsed,
            Err(ParseError {
                message: "Unterminated string literal.".to_string(),
            })
        );
    }
}
