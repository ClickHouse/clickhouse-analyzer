use tower_lsp::lsp_types::*;

use crate::analysis::scope::build_scope;
use crate::parser::diagnostic::Parse;
use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

use super::line_index::LineIndex;

pub fn handle_document_symbols(parse: &Parse, line_index: &LineIndex) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    collect_statements(&parse.tree, &parse.source, line_index, &mut symbols);
    symbols
}

fn collect_statements(
    tree: &SyntaxTree,
    source: &str,
    line_index: &LineIndex,
    symbols: &mut Vec<DocumentSymbol>,
) {
    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            if let Some(stmt_kind) = statement_label(subtree.kind) {
                let range = line_index.range(subtree.start, subtree.end);
                let scope = build_scope(subtree, source);

                let mut children = Vec::new();

                // Add CTEs
                for cte in &scope.ctes {
                    children.push(make_symbol(
                        &cte.name,
                        SymbolKind::VARIABLE,
                        "CTE",
                        line_index.range(cte.definition_range.0, cte.definition_range.1),
                        line_index.range(cte.range.0, cte.range.1),
                    ));
                }

                // Add table references
                for tref in &scope.table_refs {
                    let label = if let Some(ref alias) = tref.alias {
                        format!("{} ({})", alias, tref.table)
                    } else if let Some(ref db) = tref.database {
                        format!("{}.{}", db, tref.table)
                    } else {
                        tref.table.clone()
                    };
                    children.push(make_symbol(
                        &label,
                        SymbolKind::CLASS,
                        "table",
                        line_index.range(tref.range.0, tref.range.1),
                        line_index.range(tref.range.0, tref.range.1),
                    ));
                }

                // Add column aliases
                for alias in &scope.column_aliases {
                    children.push(make_symbol(
                        &alias.name,
                        SymbolKind::FIELD,
                        "alias",
                        line_index.range(alias.definition_range.0, alias.definition_range.1),
                        line_index.range(alias.range.0, alias.range.1),
                    ));
                }

                // Build a label from the first meaningful tokens
                let label = extract_statement_label(subtree, source, stmt_kind);

                #[allow(deprecated)]
                symbols.push(DocumentSymbol {
                    name: label,
                    detail: Some(stmt_kind.to_string()),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            } else {
                // Recurse into non-statement nodes (e.g., File, QueryList)
                collect_statements(subtree, source, line_index, symbols);
            }
        }
    }
}

#[allow(deprecated)]
fn make_symbol(
    name: &str,
    kind: SymbolKind,
    detail: &str,
    range: Range,
    selection_range: Range,
) -> DocumentSymbol {
    DocumentSymbol {
        name: name.to_string(),
        detail: Some(detail.to_string()),
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: None,
    }
}

fn statement_label(kind: SyntaxKind) -> Option<&'static str> {
    match kind {
        SyntaxKind::SelectStatement => Some("SELECT"),
        SyntaxKind::InsertStatement => Some("INSERT"),
        SyntaxKind::CreateStatement => Some("CREATE"),
        SyntaxKind::AlterStatement => Some("ALTER"),
        SyntaxKind::DeleteStatement => Some("DELETE"),
        SyntaxKind::UpdateStatement => Some("UPDATE"),
        SyntaxKind::DropStatement => Some("DROP"),
        SyntaxKind::ShowStatement => Some("SHOW"),
        SyntaxKind::ExplainStatement => Some("EXPLAIN"),
        SyntaxKind::DescribeStatement => Some("DESCRIBE"),
        SyntaxKind::UseStatement => Some("USE"),
        SyntaxKind::SetStatement => Some("SET"),
        SyntaxKind::OptimizeStatement => Some("OPTIMIZE"),
        SyntaxKind::SystemStatement => Some("SYSTEM"),
        SyntaxKind::GrantStatement => Some("GRANT"),
        SyntaxKind::RevokeStatement => Some("REVOKE"),
        SyntaxKind::TruncateStatement => Some("TRUNCATE"),
        SyntaxKind::RenameStatement => Some("RENAME"),
        _ => None,
    }
}

/// Build a short label for a statement from its first few tokens.
fn extract_statement_label(tree: &SyntaxTree, source: &str, kind_label: &str) -> String {
    let mut tokens = Vec::new();
    collect_first_tokens(tree, source, &mut tokens, 6);

    if tokens.is_empty() {
        return kind_label.to_string();
    }

    // For SELECT: show "SELECT col1, col2 FROM table"
    // For CREATE: show "CREATE TABLE name"
    // Truncate to keep it readable
    let label = tokens.join(" ");
    if label.len() > 60 {
        // Find a char boundary at or before byte 57 to avoid splitting a multi-byte character
        let mut end = 57;
        while end > 0 && !label.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &label[..end])
    } else {
        label
    }
}

fn collect_first_tokens(tree: &SyntaxTree, source: &str, tokens: &mut Vec<String>, max: usize) {
    if tokens.len() >= max {
        return;
    }
    for child in &tree.children {
        if tokens.len() >= max {
            return;
        }
        match child {
            SyntaxChild::Token(token) => {
                if token.kind == SyntaxKind::Whitespace || token.kind == SyntaxKind::Comment {
                    continue;
                }
                tokens.push(token.text(source).to_string());
            }
            SyntaxChild::Tree(subtree) => {
                collect_first_tokens(subtree, source, tokens, max);
            }
        }
    }
}
