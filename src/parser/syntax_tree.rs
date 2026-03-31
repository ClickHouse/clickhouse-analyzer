use crate::lexer::token::{Token, TokenKind};
use crate::parser::syntax_kind::SyntaxKind;
use serde::Serialize;
use std::fmt::{self, Write};

#[derive(Serialize)]
pub struct SyntaxTree {
    pub kind: SyntaxKind,
    pub children: Vec<SyntaxChild>,
}

#[derive(Serialize)]
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

    pub fn get_token_with_kind(&self, kind: TokenKind) -> Option<&Token> {
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

pub trait SyntaxChildExt {
    fn get_token_with_kind(&self, kind: TokenKind) -> Option<&Token>;
    fn get_tree_with_kind(&self, kind: SyntaxKind) -> Option<&SyntaxTree>;
}

impl SyntaxChildExt for Option<&SyntaxChild> {
    fn get_token_with_kind(&self, kind: TokenKind) -> Option<&Token> {
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
    pub fn print(&self, buf: &mut String, level: usize) {
        let indent = "  ".repeat(level);
        let _ = writeln!(buf, "{indent}{:?}", self.kind);
        for child in &self.children {
            match child {
                SyntaxChild::Token(token) => {
                    if token.kind == TokenKind::Whitespace {
                        continue;
                    }
                    let _ = writeln!(buf, "{indent}  '{}'", token.text);
                }
                SyntaxChild::Tree(tree) => tree.print(buf, level + 1),
            }
        }
        assert!(buf.ends_with('\n'));
    }
}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = String::new();
        self.print(&mut buf, 0);
        write!(f, "{}", buf)
    }
}
