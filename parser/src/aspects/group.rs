use lexer::token::{Token, TokenType};

use crate::err::ParseErrorKind;
use crate::moves::{eox, of_type, of_types, repeat, repeat_n, spaces, MoveOperations};
use crate::parser::{ParseResult, Parser};
use ast::group::{Block, Parenthesis, Subshell};
use ast::Expr;
use context::source::SourceSegment;

///A parser aspect for parsing block expressions
pub trait GroupAspect<'a> {
    ///Parse a block expression.
    /// Block expressions will parse contained expressions as statements.
    /// see `Parser::statement` for further details.
    fn block(&mut self) -> ParseResult<Block<'a>>;

    ///Parse a subshell expression.
    /// subshell expressions will parse contained expressions as statements.
    /// see [`Parser::statement`] for further details.
    fn subshell(&mut self) -> ParseResult<Subshell<'a>>;

    ///Parse a parenthesis (or grouped value) expression.
    /// parenthesis expressions will parse contained expression as a value.
    /// Thus, a parenthesis group is not meant to
    /// see `Parser::statement` for further details.
    fn parenthesis(&mut self) -> ParseResult<Parenthesis<'a>>;
}

impl<'a> GroupAspect<'a> for Parser<'a> {
    fn block(&mut self) -> ParseResult<Block<'a>> {
        let start = self.ensure_at_group_start(TokenType::CurlyLeftBracket)?;
        let (expressions, segment) =
            self.sub_exprs(start, TokenType::CurlyRightBracket, Parser::statement)?;
        Ok(Block {
            expressions,
            segment,
        })
    }

    fn subshell(&mut self) -> ParseResult<Subshell<'a>> {
        let start = self.ensure_at_group_start(TokenType::RoundedLeftBracket)?;
        let (expressions, segment) = self.sub_exprs(
            start.clone(),
            TokenType::RoundedRightBracket,
            Parser::statement,
        )?;
        Ok(Subshell {
            expressions,
            segment,
        })
    }

    fn parenthesis(&mut self) -> ParseResult<Parenthesis<'a>> {
        let start = self.ensure_at_group_start(TokenType::RoundedLeftBracket)?;
        let expr = self.value().map_err(|err| {
            self.repos_to_top_delimiter();
            err
        })?;
        let end = self
            .cursor
            .force(
                repeat(spaces().then(eox())) //consume possible end of expressions
                    .then(spaces().then(of_type(TokenType::RoundedRightBracket))), //expect closing ')' token
                "parenthesis in value expression can only contain one expression",
            )
            .map_err(|err| {
                self.repos_to_top_delimiter();
                err
            })?;
        self.delimiter_stack.pop_back();

        Ok(Parenthesis {
            expression: Box::new(expr),
            segment: self.cursor.relative_pos_ctx(start..end),
        })
    }
}

impl<'a> Parser<'a> {
    fn ensure_at_group_start(&mut self, start: TokenType) -> ParseResult<Token<'a>> {
        let token = self.cursor.force_with(
            of_type(start),
            "Unexpected start of group expression",
            ParseErrorKind::Expected(start.str().unwrap_or("specific token").to_string()),
        )?; //consume group start token
        self.delimiter_stack.push_back(token.clone());
        Ok(token)
    }

    ///parses sub expressions of a grouping expression
    fn sub_exprs<F>(
        &mut self,
        start_token: Token,
        eog: TokenType,
        mut parser: F,
    ) -> ParseResult<(Vec<Expr<'a>>, SourceSegment)>
    where
        F: FnMut(&mut Self) -> ParseResult<Expr<'a>>,
    {
        let mut statements: Vec<Expr<'a>> = Vec::new();
        let mut segment = self.cursor.relative_pos(start_token.value);

        //consume all heading spaces and end of expressions (\n or ;)
        self.cursor.advance(repeat(of_types(&[
            TokenType::Space,
            TokenType::NewLine,
            TokenType::SemiColon,
        ])));

        //if we directly hit end of group, return an empty block.
        if let Some(eog) = self.cursor.advance(of_type(eog)) {
            self.delimiter_stack.pop_back();
            return Ok((statements, segment.start..self.cursor.relative_pos(eog).end));
        }

        loop {
            if self.cursor.is_at_end() {
                self.expected(
                    "Expected closing bracket.",
                    ParseErrorKind::Unpaired(self.cursor.relative_pos(&start_token)),
                )?;
            }
            let statement = parser(self);
            match statement {
                Ok(statement) => {
                    statements.push(statement);
                }
                Err(err) => {
                    self.recover_from(err, eox());
                }
            }

            //expects at least one newline or ';'
            let eox_res = self.cursor.force(
                repeat_n(1, spaces().then(eox())),
                "expected new line or semicolon",
            );

            //checks if this group expression is closed after the parsed expression
            let closed = self.cursor.advance(spaces().then(of_type(eog)));

            //if the group is closed, then we stop looking for other expressions.
            if let Some(closing) = closed {
                self.delimiter_stack.pop_back();
                segment = segment.start..self.cursor.relative_pos(closing).end;
                break;
            }

            if eox_res.is_err() && self.cursor.peek().token_type.is_closing_ponctuation() {
                self.mismatched_delimiter(eog)?;
            }

            //but if not closed, expect the cursor to hit EOX.
            eox_res?;
        }
        Ok((statements, segment))
    }
}

#[cfg(test)]
mod tests {
    use crate::aspects::group::GroupAspect;
    use crate::parser::Parser;
    use crate::source::literal;
    use ast::call::Call;
    use ast::group::{Block, Subshell};
    use ast::r#type::ParametrizedType;
    use ast::r#type::Type;
    use ast::value::Literal;
    use ast::value::LiteralValue::{Float, Int};
    use ast::variable::{TypedVariable, VarDeclaration, VarKind};
    use ast::Expr;
    use context::source::{Source, SourceSegmentHolder};
    use context::str_find::{find_between, find_in, find_in_nth};
    use pretty_assertions::assert_eq;

    //noinspection DuplicatedCode
    #[test]
    fn empty_blocks() {
        let source = Source::unknown("{{{}; {}}}");
        let mut parser = Parser::new(source.clone());
        let ast = parser.block().expect("failed to parse block");
        assert!(parser.cursor.is_at_end());
        assert_eq!(
            ast,
            Block {
                expressions: vec![Expr::Block(Block {
                    expressions: vec![
                        Expr::Block(Block {
                            expressions: vec![],
                            segment: 2..4
                        }),
                        Expr::Block(Block {
                            expressions: vec![],
                            segment: 6..8
                        }),
                    ],
                    segment: 1..source.source.len() - 1
                })],
                segment: source.segment()
            }
        );
    }

    //noinspection DuplicatedCode
    #[test]
    fn empty_blocks_empty_content() {
        let source = Source::unknown("{;;{;;;{;;}; {\n\n};}}");
        let mut parser = Parser::new(source.clone());
        let ast = parser.block().expect("failed to parse block");
        assert!(parser.cursor.is_at_end());
        assert_eq!(
            ast,
            Block {
                expressions: vec![Expr::Block(Block {
                    expressions: vec![
                        Expr::Block(Block {
                            expressions: vec![],
                            segment: 7..11
                        }),
                        Expr::Block(Block {
                            expressions: vec![],
                            segment: 13..17
                        }),
                    ],
                    segment: 3..source.source.len() - 1
                })],
                segment: source.segment()
            }
        );
    }

    #[test]
    fn blank_block() {
        let source = Source::unknown("{ }");
        let result = Parser::new(source.clone()).parse_specific(|parser| parser.block());
        assert_eq!(
            result.expect("failed to parse block"),
            Block {
                expressions: vec![],
                segment: source.segment()
            }
        );
    }

    #[test]
    fn block_not_ended() {
        let source = Source::unknown("{ val test = 2 ");
        let mut parser = Parser::new(source);
        parser.block().expect_err("block parse did not failed");
    }

    #[test]
    fn neighbour_parenthesis() {
        let source = Source::unknown("{ {} {} }");
        let mut parser = Parser::new(source);
        parser.block().expect_err("block parse did not failed");
    }

    #[test]
    fn block_not_started() {
        let source = Source::unknown(" val test = 2 }");
        let mut parser = Parser::new(source);
        parser.block().expect_err("block parse did not failed");
    }

    #[test]
    fn block_with_nested_blocks() {
        let source = Source::unknown(
            "{\
            val test = {\
                val x = 8\n\n\n
                8
            }\n\
            (val x = 89; command call; 7)\
        }",
        );
        let mut parser = Parser::new(source.clone());
        let ast = parser
            .block()
            .expect("failed to parse block with nested blocks");
        assert!(parser.cursor.is_at_end());
        assert_eq!(
            ast,
            Block {
                expressions: vec![
                    Expr::VarDeclaration(VarDeclaration {
                        kind: VarKind::Val,
                        var: TypedVariable {
                            name: "test",
                            ty: None,
                            segment: find_in(source.source, "test"),
                        },
                        initializer: Some(Box::from(Expr::Block(Block {
                            expressions: vec![
                                Expr::VarDeclaration(VarDeclaration {
                                    kind: VarKind::Val,
                                    var: TypedVariable {
                                        name: "x",
                                        ty: None,
                                        segment: find_in(source.source, "x"),
                                    },
                                    initializer: Some(Box::from(Expr::Literal(Literal {
                                        parsed: Int(8),
                                        segment: find_in(source.source, "8"),
                                    }))),
                                    segment: find_in(source.source, "val x = 8"),
                                }),
                                Expr::Literal(Literal {
                                    parsed: Int(8),
                                    segment: find_in_nth(source.source, "8", 1),
                                }),
                            ],
                            segment: find_between(
                                source.source,
                                "{\
                val x",
                                "}"
                            ),
                        }))),
                        segment: find_between(source.source, "val test = {", "}"),
                    }),
                    Expr::Subshell(Subshell {
                        expressions: vec![
                            Expr::VarDeclaration(VarDeclaration {
                                kind: VarKind::Val,
                                var: TypedVariable {
                                    name: "x",
                                    ty: None,
                                    segment: find_in_nth(source.source, "x", 1),
                                },
                                initializer: Some(Box::from(Expr::Literal(Literal {
                                    parsed: Int(89),
                                    segment: find_in(source.source, "89")
                                }))),
                                segment: find_in(source.source, "val x = 89"),
                            }),
                            Expr::Call(Call {
                                path: Vec::new(),
                                arguments: vec![
                                    literal(source.source, "command"),
                                    literal(source.source, "call")
                                ],
                                type_parameters: vec![],
                            }),
                            Expr::Literal(Literal {
                                parsed: Int(7),
                                segment: find_in(source.source, "7")
                            })
                        ],
                        segment: find_between(source.source, "(", ")"),
                    }),
                ],
                segment: source.segment()
            }
        )
    }

    #[test]
    fn block() {
        let source = Source::unknown(
            "{\
            var test: int = 7.0\n\
            val x = 8\
        }",
        );
        let mut parser = Parser::new(source.clone());
        let ast = parser.block().expect("failed to parse block");
        assert!(parser.cursor.is_at_end());
        assert_eq!(
            ast,
            Block {
                expressions: vec![
                    Expr::VarDeclaration(VarDeclaration {
                        kind: VarKind::Var,
                        var: TypedVariable {
                            name: "test",
                            ty: Some(Type::Parametrized(ParametrizedType {
                                path: vec![],
                                name: "int",
                                params: Vec::new(),
                                segment: find_in(source.source, "int")
                            })),
                            segment: find_in(source.source, "test: int")
                        },
                        initializer: Some(Box::new(Expr::Literal(Literal {
                            parsed: Float(7.0),
                            segment: find_in(source.source, "7.0")
                        }))),
                        segment: find_in(source.source, "var test: int = 7.0"),
                    }),
                    Expr::VarDeclaration(VarDeclaration {
                        kind: VarKind::Val,
                        var: TypedVariable {
                            name: "x",
                            ty: None,
                            segment: find_in(source.source, "x"),
                        },
                        initializer: Some(Box::new(Expr::Literal(Literal {
                            parsed: Int(8),
                            segment: find_in(source.source, "8"),
                        }))),
                        segment: find_in(source.source, "val x = 8"),
                    }),
                ],
                segment: source.segment()
            }
        )
    }
}
