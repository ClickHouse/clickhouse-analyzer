use std::fmt;

/// ClickHouse Tokens, same as the original
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Base tokens
    Whitespace,
    Comment,

    BareWord,       // Keywords or identifiers
    Number,         // Numeric literals
    StringLiteral,  // String literals with single quotes
    QuotedIdentifier, // Double-quoted or backtick-quoted identifiers

    // Brackets
    OpeningRoundBracket,
    ClosingRoundBracket,
    OpeningSquareBracket,
    ClosingSquareBracket,
    OpeningCurlyBrace,
    ClosingCurlyBrace,

    // Punctuation
    Comma,
    Semicolon,
    VerticalDelimiter, // \G
    Dot,

    // Operators and special symbols
    Asterisk,
    HereDoc,
    DollarSign,
    Plus,
    Minus,
    Slash,
    Percent,
    Arrow,
    QuestionMark,
    Colon,
    Caret,
    DoubleColon,
    Equals,
    NotEquals,
    Less,
    Greater,
    LessOrEquals,
    GreaterOrEquals,
    Spaceship,     // <=>
    PipeMark,
    Concatenation, // ||

    // MySQL-style variables
    At,
    DoubleAt,

    // End of stream
    EndOfStream,

    // Error tokens
    Error,
    ErrorMultilineCommentIsNotClosed,
    ErrorSingleQuoteIsNotClosed,
    ErrorDoubleQuoteIsNotClosed,
    ErrorBackQuoteIsNotClosed,
    ErrorSingleExclamationMark,
    ErrorSinglePipeMark,
    ErrorWrongNumber,
    ErrorMaxQuerySizeExceeded,
    
    // Temporary hack for WHERE operators
    And, // AND
    Or // OR
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::BareWord => write!(f, "identifier or keyword"),
            TokenKind::Number => write!(f, "number"),
            TokenKind::StringLiteral => write!(f, "string literal"),
            TokenKind::QuotedIdentifier => write!(f, "quoted identifier"),
            TokenKind::OpeningRoundBracket => write!(f, "("),
            TokenKind::ClosingRoundBracket => write!(f, ")"),
            // All other token types
            _ => write!(f, "{:?}", self),
        }
    }
}

/// Structure representing a token in the SQL
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub start: usize,  // Start position in the source
    pub end: usize,    // End position in the source
    pub line: usize,   // Line number
    pub column: usize, // Column number
}

impl Token {
    pub fn new(kind: TokenKind, value: String, start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            kind,
            text: value,
            start,
            end,
            line,
            column,
        }
    }
}
