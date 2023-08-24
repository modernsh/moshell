use ast::r#use::{Import, ImportList, ImportedSymbol, InclusionPathItem, Use};
use ast::Expr;
use context::source::SourceSegmentHolder;
use lexer::token::TokenType::{
    As, At, ColonColon, CurlyLeftBracket, CurlyRightBracket, Identifier, Reef, Star,
};
use lexer::token::{Token, TokenType};

use crate::aspects::expr_list::ExpressionListAspect;
use crate::err::ParseErrorKind;
use crate::moves::{any, blanks, eox, of_type, of_types, spaces, MoveOperations};
use crate::parser::{ParseResult, Parser};

/// Parser aspect to parse all expressions in relation with modules.
/// Which can be use statements, or module location prefix
pub trait ModulesAspect<'a> {
    ///parse a 'use x, y' statement
    fn parse_use(&mut self) -> ParseResult<Expr<'a>>;

    ///parse identifiers separated between `::` expressions.
    /// This method stops when it founds an expressions that is not an identifier.
    fn parse_inclusion_path(&mut self) -> ParseResult<Vec<InclusionPathItem<'a>>>;
}

impl<'a> ModulesAspect<'a> for Parser<'a> {
    fn parse_use(&mut self) -> ParseResult<Expr<'a>> {
        let start = self
            .cursor
            .force(of_type(TokenType::Use), "expected 'use'")?;

        let import = self.parse_import()?;

        self.cursor.advance(spaces()); //consume spaces

        if self.cursor.lookahead(eox()).is_none() {
            return self.expected(
                "expected new line or semicolon",
                ParseErrorKind::Expected("<new_line> or ';'".to_string()),
            );
        };

        let import_seg_end = import.segment().end;
        Ok(Expr::Use(Use {
            import,
            segment: self.cursor.relative_pos_ctx(start).start..import_seg_end,
        }))
    }

    fn parse_inclusion_path(&mut self) -> ParseResult<Vec<InclusionPathItem<'a>>> {
        let mut items = vec![];

        while let Some(identifier) = self
            .cursor
            .lookahead(spaces().then(of_types(&[Reef, Identifier])))
        {
            if self
                .cursor
                .advance(any().then(spaces().then(of_type(ColonColon))))
                .is_none()
            {
                break;
            }
            let segment = self.cursor.relative_pos_ctx(identifier.value);
            let item = match identifier.token_type {
                Identifier => InclusionPathItem::Symbol(identifier.value, segment),
                Reef => InclusionPathItem::Reef(segment),
                _ => unreachable!(),
            };
            items.push(item);
        }

        Ok(items)
    }
}

impl<'a> Parser<'a> {
    fn parse_import(&mut self) -> ParseResult<Import<'a>> {
        self.cursor.advance(blanks()); //consume blanks

        let pivot = self.cursor.peek();

        match pivot.token_type {
            At => {
                //handle '@ENV_VARIABLE` expression
                self.cursor.next()?;
                let env_variable = self.cursor.force_with(
                    spaces().then(of_type(Identifier)),
                    "Environment variable name expected.",
                    ParseErrorKind::Expected("<identifier>".to_string()),
                )?;
                Ok(Import::Environment(
                    env_variable.value,
                    self.cursor.relative_pos_ctx(pivot..env_variable),
                ))
            }
            Star => self.expected_with(
                "import all statement needs a symbol prefix.",
                pivot,
                ParseErrorKind::Expected("module path".to_string()),
            ),
            CurlyLeftBracket => self.parse_import_list(pivot, vec![]).map(Import::List),

            _ => self.parse_import_with_path(),
        }
    }

    fn parse_import_list(
        &mut self,
        start: Token<'a>,
        root: Vec<InclusionPathItem<'a>>,
    ) -> ParseResult<ImportList<'a>> {
        self.parse_explicit_list(CurlyLeftBracket, CurlyRightBracket, Self::parse_import)
            .and_then(|(imports, s)| {
                if imports.is_empty() {
                    return self.expected_with(
                        "empty brackets",
                        start..self.cursor.peek(),
                        ParseErrorKind::Expected("non-empty brackets".to_string()),
                    );
                }
                Ok(ImportList {
                    root,
                    imports,
                    segment: self.cursor.relative_pos_ctx(start).start..s.end,
                })
            })
    }

    fn expect_identifier(&mut self) -> ParseResult<&'a str> {
        self.cursor.advance(spaces()); //consume spaces

        self.cursor
            .force_with(
                of_type(Identifier),
                "identifier expected, you might want to escape it",
                ParseErrorKind::UnexpectedInContext(format!("\\{}", self.cursor.peek().value)),
            )
            .map(|t| t.value)
    }

    fn parse_import_with_path(&mut self) -> ParseResult<Import<'a>> {
        let start = self.cursor.peek();

        let mut symbol_path = self.parse_inclusion_path()?;
        self.cursor.advance(spaces()); //consume spaces
        let token = self.cursor.peek();

        if token.token_type == Star || token.token_type == CurlyLeftBracket {
            if token.token_type == Star {
                self.cursor.next()?;

                return Ok(Import::AllIn(
                    symbol_path,
                    self.cursor.relative_pos_ctx(start..token),
                ));
            }
            return self.parse_import_list(start, symbol_path).map(Import::List);
        }

        let name = if token.token_type == Identifier {
            self.cursor.next()?;
            token.value
        } else {
            return self.expected(
                "identifier expected",
                ParseErrorKind::Expected("<identifier>".to_string()),
            );
        };

        let alias = self.cursor.advance(
            spaces()
                .then(of_type(As))
                .and_then(spaces())
                .then(of_type(Identifier)),
        );

        let end = alias.clone().unwrap_or(token);

        symbol_path.push(InclusionPathItem::Symbol(
            name,
            self.cursor.relative_pos_ctx(name),
        ));

        Ok(Import::Symbol(ImportedSymbol {
            path: symbol_path,
            alias: alias.map(|t| t.value),
            segment: self.cursor.relative_pos_ctx(start..end),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use pretty_assertions::assert_eq;

    use ast::r#use::{Import, ImportList, ImportedSymbol, InclusionPathItem, Use};
    use ast::Expr;
    use context::source::{Source, SourceSegmentHolder};
    use context::str_find::{find_in, find_in_nth};

    use crate::err::{ParseError, ParseErrorKind};
    use crate::parse;
    use crate::parser::ParseResult;

    #[test]
    fn simple_use() {
        let source = Source::unknown("use reef::std::foo::bar");
        let result = parse(source).expect("parser failed");
        assert_eq!(
            result,
            vec![Expr::Use(Use {
                import: Import::Symbol(ImportedSymbol {
                    path: vec![
                        InclusionPathItem::Reef(find_in(source.source, "reef")),
                        InclusionPathItem::Symbol("std", find_in(source.source, "std")),
                        InclusionPathItem::Symbol("foo", find_in(source.source, "foo")),
                        InclusionPathItem::Symbol("bar", find_in(source.source, "bar")),
                    ],
                    alias: None,
                    segment: find_in(source.source, "reef::std::foo::bar"),
                }),
                segment: source.segment(),
            })]
        )
    }

    #[test]
    fn wrong_env_name() {
        let source = Source::unknown("use @9");
        let result: ParseResult<_> = parse(source).into();
        assert_eq!(
            result,
            Err(ParseError {
                message: "Environment variable name expected.".to_string(),
                kind: ParseErrorKind::Expected("<identifier>".to_string()),
                position: source.source.len() - 1..source.source.len(),
            })
        )
    }

    #[test]
    fn list_use_aliased() {
        let source = Source::unknown("use std::foo::{bar} as X");
        let result: ParseResult<_> = parse(source).into();
        assert_eq!(
            result,
            Err(ParseError {
                message: "expected new line or semicolon".to_string(),
                kind: ParseErrorKind::Expected("<new_line> or ';'".to_string()),
                position: source.source.find("as").map(|i| i..i + 2).unwrap(),
            })
        )
    }

    #[test]
    fn use_empty_list() {
        let source = Source::unknown("use {}");
        let result: ParseResult<_> = parse(source).into();
        assert_eq!(
            result,
            Err(ParseError {
                message: "empty brackets".to_string(),
                kind: ParseErrorKind::Expected("non-empty brackets".to_string()),
                position: source.source.find("{}").map(|i| i..i + 2).unwrap(),
            })
        )
    }

    #[test]
    fn use_all() {
        let source = Source::unknown("use *");
        let result: ParseResult<_> = parse(source).into();
        assert_eq!(
            result,
            Err(ParseError {
                message: "import all statement needs a symbol prefix.".to_string(),
                kind: ParseErrorKind::Expected("module path".to_string()),
                position: source.source.find("*").map(|i| i..i + 1).unwrap(),
            })
        )
    }

    #[test]
    fn use_all_in() {
        let source = Source::unknown("use std::*");
        let result = parse(source).expect("parser failed");
        assert_eq!(
            result,
            vec![Expr::Use(Use {
                import: Import::AllIn(
                    vec![InclusionPathItem::Symbol(
                        "std",
                        find_in(source.source, "std")
                    ),],
                    find_in(source.source, "std::*"),
                ),
                segment: source.segment(),
            })]
        )
    }

    #[test]
    fn uses() {
        let source = Source::unknown(
            "use \nreef::{std::TOKEN as X,    @A \n , @B \\\n , reef::foo::{my_function}}",
        );
        let result = parse(source).expect("parser failed");
        assert_eq!(
            result,
            vec![Expr::Use(Use {
                segment: source.segment(),
                import: Import::List(ImportList {
                    root: vec![InclusionPathItem::Reef(find_in(source.source, "reef"))],
                    segment: find_in(
                        source.source,
                        "reef::{std::TOKEN as X,    @A \n , @B \\\n , reef::foo::{my_function}}"
                    ),
                    imports: vec![
                        Import::Symbol(ImportedSymbol {
                            path: vec![
                                InclusionPathItem::Symbol("std", find_in(source.source, "std")),
                                InclusionPathItem::Symbol("TOKEN", find_in(source.source, "TOKEN"))
                            ],
                            alias: Some("X"),
                            segment: find_in(source.source, "std::TOKEN as X"),
                        }),
                        Import::Environment("A", find_in(source.source, "@A")),
                        Import::Environment("B", find_in(source.source, "@B")),
                        Import::List(ImportList {
                            root: vec![
                                InclusionPathItem::Reef(find_in_nth(source.source, "reef", 1)),
                                InclusionPathItem::Symbol("foo", find_in(source.source, "foo"))
                            ],
                            segment: find_in(source.source, "reef::foo::{my_function}"),
                            imports: vec![Import::Symbol(ImportedSymbol {
                                path: vec![InclusionPathItem::Symbol(
                                    "my_function",
                                    find_in(source.source, "my_function")
                                )],
                                alias: None,
                                segment: find_in(source.source, "my_function"),
                            }),],
                        }),
                    ],
                })
            })]
        )
    }

    #[test]
    fn use_trailing_comma() {
        let content = "use TOKEN, A";
        let source = Source::unknown(content);
        let result: ParseResult<_> = parse(source).into();
        assert_eq!(
            result,
            Err(ParseError {
                message: "expected new line or semicolon".to_string(),
                position: content.find(',').map(|p| p..p + 1).unwrap(),
                kind: ParseErrorKind::Expected("<new_line> or ';'".to_string()),
            })
        )
    }

    #[test]
    fn use_empty() {
        let content = "use";
        let source = Source::unknown(content);
        let result: ParseResult<_> = parse(source).into();
        assert_eq!(
            result,
            Err(ParseError {
                message: "identifier expected".to_string(),
                position: content.len()..content.len(),
                kind: ParseErrorKind::Expected("<identifier>".to_string()),
            })
        )
    }
}
