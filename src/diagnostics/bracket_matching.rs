use super::types::{Diagnostic, RelatedSpan};
use crate::parser::syntax_tree::{SyntaxTree, SyntaxChild};
use crate::parser::syntax_kind::SyntaxKind;

struct BracketInfo {
    kind: SyntaxKind,
    range: (usize, usize),
}

fn collect_brackets(tree: &SyntaxTree, stack: &mut Vec<BracketInfo>) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                match token.kind {
                    SyntaxKind::OpeningRoundBracket
                    | SyntaxKind::OpeningSquareBracket
                    | SyntaxKind::OpeningCurlyBrace => {
                        stack.push(BracketInfo {
                            kind: token.kind,
                            range: (token.start as usize, token.end as usize),
                        });
                    }
                    SyntaxKind::ClosingRoundBracket
                    | SyntaxKind::ClosingSquareBracket
                    | SyntaxKind::ClosingCurlyBrace => {
                        // Pop matching opener
                        if let Some(last) = stack.last() {
                            let matches = matches!(
                                (last.kind, token.kind),
                                (SyntaxKind::OpeningRoundBracket, SyntaxKind::ClosingRoundBracket)
                                    | (SyntaxKind::OpeningSquareBracket, SyntaxKind::ClosingSquareBracket)
                                    | (SyntaxKind::OpeningCurlyBrace, SyntaxKind::ClosingCurlyBrace)
                            );
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
            Some(SyntaxKind::OpeningRoundBracket)
        } else if diag.message.contains("expected ]") {
            Some(SyntaxKind::OpeningSquareBracket)
        } else if diag.message.contains("expected }") {
            Some(SyntaxKind::OpeningCurlyBrace)
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
