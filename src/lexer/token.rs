use crate::parser::syntax_kind::SyntaxKind;

/// Structure representing a token in the SQL
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Token {
    pub kind: SyntaxKind,
    pub start: u32, // Start byte offset in the source
    pub end: u32,   // End byte offset in the source
}

impl Token {
    pub fn new(kind: SyntaxKind, start: u32, end: u32) -> Self {
        Self { kind, start, end }
    }

    /// Retrieve the token text by slicing from the source string.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }
}
