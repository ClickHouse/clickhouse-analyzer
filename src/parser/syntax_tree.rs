use crate::lexer::token::Token;
use crate::parser::syntax_kind::SyntaxKind;
use std::fmt::Write;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SyntaxTree {
    pub kind: SyntaxKind,
    pub children: Vec<SyntaxChild>,
    /// Byte offset of the first token in this subtree (u32::MAX if empty).
    pub start: u32,
    /// Byte offset of the end of the last token in this subtree (0 if empty).
    pub end: u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum SyntaxChild {
    Token(Token),
    Tree(SyntaxTree),
}

impl SyntaxChild {
    pub fn is_token(&self) -> bool {
        matches!(self, SyntaxChild::Token(_))
    }

    pub fn is_tree(&self) -> bool {
        matches!(self, SyntaxChild::Tree(_))
    }

    pub fn get_token_with_kind(&self, kind: SyntaxKind) -> Option<&Token> {
        match self {
            SyntaxChild::Token(token) if token.kind == kind => Some(token),
            _ => None,
        }
    }

    pub fn get_tree_with_kind(&self, kind: SyntaxKind) -> Option<&SyntaxTree> {
        match self {
            SyntaxChild::Tree(tree) if tree.kind == kind => Some(tree),
            _ => None,
        }
    }
}

#[allow(dead_code)]
pub trait SyntaxChildExt {
    fn get_token_with_kind(&self, kind: SyntaxKind) -> Option<&Token>;
    fn get_tree_with_kind(&self, kind: SyntaxKind) -> Option<&SyntaxTree>;
}

impl SyntaxChildExt for Option<&SyntaxChild> {
    fn get_token_with_kind(&self, kind: SyntaxKind) -> Option<&Token> {
        match self {
            Some(SyntaxChild::Token(token)) if token.kind == kind => Some(token),
            _ => None,
        }
    }

    fn get_tree_with_kind(&self, kind: SyntaxKind) -> Option<&SyntaxTree> {
        match self {
            Some(SyntaxChild::Tree(tree)) if tree.kind == kind => Some(tree),
            _ => None,
        }
    }
}

impl SyntaxTree {
    pub fn print(&self, buf: &mut String, level: usize, source: &str) {
        let indent = "  ".repeat(level);
        let _ = writeln!(buf, "{indent}{:?}", self.kind);
        for child in &self.children {
            match child {
                SyntaxChild::Token(token) => {
                    if token.kind == SyntaxKind::Whitespace {
                        continue;
                    }
                    let _ = writeln!(buf, "{indent}  '{}'", token.text(source));
                }
                SyntaxChild::Tree(tree) => tree.print(buf, level + 1, source),
            }
        }
        // Invariant: print always ends with a newline (from writeln above).
        // Use debug_assert to catch violations during development without
        // crashing production callers.
        debug_assert!(buf.ends_with('\n'));
    }
}
