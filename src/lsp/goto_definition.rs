use tower_lsp::lsp_types::*;

use crate::analysis::scope::{build_scope, find_enclosing_statement};
use crate::parser::diagnostic::Parse;
use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

use super::line_index::LineIndex;

pub fn handle_goto_definition(
    parse: &Parse,
    line_index: &LineIndex,
    position: Position,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    let offset = line_index.offset(position);

    // Find the token at cursor
    let (text, _parent, _start, _end) = find_token_at(&parse.tree, &parse.source, offset)?;

    // Find the enclosing statement and build scope
    let stmt = find_enclosing_statement(&parse.tree, offset)?;
    let scope = build_scope(stmt, &parse.source);

    // Check CTEs
    if let Some(cte) = scope
        .ctes
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(text))
    {
        return Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: line_index.range(cte.definition_range.0, cte.definition_range.1),
        }));
    }

    // Check table aliases
    if let Some(alias) = scope
        .table_aliases
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(text))
    {
        return Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: line_index.range(alias.range.0, alias.range.1),
        }));
    }

    // Check column aliases
    if let Some(alias) = scope
        .column_aliases
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(text))
    {
        return Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: line_index.range(alias.definition_range.0, alias.definition_range.1),
        }));
    }

    None
}

fn find_token_at<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    offset: u32,
) -> Option<(&'a str, SyntaxKind, u32, u32)> {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                if token.kind == SyntaxKind::Whitespace || token.kind == SyntaxKind::Comment {
                    continue;
                }
                if token.start <= offset && offset <= token.end {
                    return Some((token.text(source), token.kind, token.start, token.end));
                }
            }
            SyntaxChild::Tree(subtree) => {
                if subtree.start > subtree.end {
                    continue;
                }
                if subtree.start <= offset && offset <= subtree.end {
                    if let Some(result) = find_token_at(subtree, source, offset) {
                        return Some(result);
                    }
                }
            }
        }
    }
    None
}
