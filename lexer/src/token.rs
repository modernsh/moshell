use logos::Logos;

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub token_type: TokenType,
    pub value: &'a str,
}

impl<'a> Token<'a> {
    pub fn new(token_type: TokenType, value: &'a str) -> Self {
        Self { token_type, value }
    }
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
pub enum TokenType {
    #[token("var")]
    Var,
    #[token("val")]
    Val,

    #[regex("[\\./\\p{XID_Start}](?:[^\\s'\"$@:}]|\\\\.)*")]
    Identifier,

    #[regex("-?[0-9]+", priority = 2)]
    IntLiteral,
    #[regex("-?[0-9]+\\.[0-9]+")]
    FloatLiteral,

    #[token("\n")]
    NewLine,

    #[token("fun")]
    Fun,
    #[token("use")]
    Use,
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("while")]
    While,
    #[token("match")]
    Match,

    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,

    #[token(":")]
    Colon,
    #[token("=")]
    Equal,
    #[token("'")]
    Quote,
    #[token("\"")]
    DoubleQuote,
    #[token("$")]
    Dollar,
    #[token("&")]
    Ampersand,
    #[token("@")]
    At,

    #[token("|")]
    Pipe,

    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("!")]
    Not,

    #[token("==")]
    EqualEqual,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    Less,
    #[token("<=")]
    LessEqual,
    #[token(">")]
    Greater,
    #[token(">=")]
    GreaterEqual,

    #[token("+=")]
    PlusEqual,
    #[token("-=")]
    MinusEqual,
    #[token("*=")]
    TimesEqual,
    #[token("/=")]
    DivideEqual,
    #[token("%=")]
    ModuloEqual,

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Times,
    #[token("/")]
    Divide,
    #[token("%")]
    Modulo,

    #[token("[")]
    SquareLeftBracket,
    #[token("]")]
    SquareRightBracket,
    #[token("(")]
    RoundedLeftBracket,
    #[token(")")]
    RoundedRightBracket,
    #[token("{")]
    CurlyLeftBracket,
    #[token("}")]
    CurlyRightBracket,

    #[regex(r"[ \t\f]+")]
    Space,

    #[regex("//.*", logos::skip)]
    #[error]
    Error,

    EndOfFile,
}

impl TokenType {
    pub fn is_ponctuation(self) -> bool {
        matches!(
            self,
            TokenType::Ampersand
                | TokenType::Less
                | TokenType::Greater
                | TokenType::Pipe
                | TokenType::SquareLeftBracket
                | TokenType::SquareRightBracket
                | TokenType::RoundedLeftBracket
                | TokenType::RoundedRightBracket
                | TokenType::CurlyLeftBracket
                | TokenType::CurlyRightBracket
                | TokenType::Space
                | TokenType::Error
        )
    }
}
