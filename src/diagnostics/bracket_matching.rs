use super::types::{Diagnostic, RelatedSpan};
use crate::parser::syntax_tree::{SyntaxTree, SyntaxChild};
use crate::lexer::token::TokenKind;

struct BracketInfo {
    kind: TokenKind,
    range: (usize, usize),
}

fn collect_brackets(tree: &SyntaxTree, stack: &mut Vec<BracketInfo>) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                match token.kind {
                    TokenKind::OpeningRoundBracket
                    | TokenKind::OpeningSquareBracket
                    | TokenKind::OpeningCurlyBrace => {
                        stack.push(BracketInfo {
                            kind: token.kind,
                            range: (token.start, token.end),
                        });
                    }
                    TokenKind::ClosingRoundBracket
                    | TokenKind::ClosingSquareBracket
                    | TokenKind::ClosingCurlyBrace => {
                        // Pop matching opener
                        if let Some(last) = stack.last() {
                            let matches = match (last.kind, token.kind) {
                                (TokenKind::OpeningRoundBracket, TokenKind::ClosingRoundBracket) => true,
                                (TokenKind::OpeningSquareBracket, TokenKind::ClosingSquareBracket) => true,
                                (TokenKind::OpeningCurlyBrace, TokenKind::ClosingCurlyBrace) => true,
                                _ => false,
                            };
                            if matches {
                                stack.pop();
                            }
                        }
                    }
                    _ => {}
                }
            }
            SyntaxChild::Tree(subtree) => {
                collect_brackets(subtree, stack);
            }
        }
    }
}

pub fn enrich(diagnostics: &mut [Diagnostic], tree: &SyntaxTree) {
    // Collect all unmatched opening brackets
    let mut stack = Vec::new();
    collect_brackets(tree, &mut stack);

    // For each "expected )" / "]" / "}" diagnostic, find the matching opener
    for diag in diagnostics.iter_mut() {
        let closer = if diag.message.contains("expected )") {
            Some(TokenKind::OpeningRoundBracket)
        } else if diag.message.contains("expected ]") {
            Some(TokenKind::OpeningSquareBracket)
        } else if diag.message.contains("expected }") {
            Some(TokenKind::OpeningCurlyBrace)
        } else {
            None
        };

        if let Some(opener_kind) = closer {
            // Find the nearest unmatched opener of this type
            if let Some(opener) = stack.iter().rev().find(|b| b.kind == opener_kind) {
                diag.related.push(RelatedSpan {
                    range: opener.range,
                    message: "Unclosed bracket opened here".to_string(),
                });
            }
        }
    }
}
