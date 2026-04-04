use tower_lsp::lsp_types::*;

use crate::analysis::scope::build_scope;
use crate::metadata::cache::SharedMetadata;
use crate::parser::diagnostic::Parse;
use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

use super::line_index::LineIndex;

pub async fn handle_hover(
    parse: &Parse,
    line_index: &LineIndex,
    position: Position,
    metadata: &SharedMetadata,
) -> Option<Hover> {
    let offset = line_index.offset(position);
    let (text, parent_kind, token_start, token_end) =
        find_token_with_parent(&parse.tree, &parse.source, offset)?;

    // Pre-fetch columns if hovering on a table identifier (needs write lock)
    if parent_kind == SyntaxKind::TableIdentifier || parent_kind == SyntaxKind::FromClause {
        let mut meta = metadata.write().await;
        if meta.is_connected() {
            // Resolve table — could be db.table or just table
            let default_db = meta.default_database().to_string();
            let scope = build_scope(&parse.tree, &parse.source);
            let (db, table) = resolve_table_for_hover(text, &scope, &default_db);
            let _ = meta.ensure_columns(&db, &table).await;
        }
    }

    let meta = metadata.read().await;

    let contents = match parent_kind {
        SyntaxKind::FunctionCall | SyntaxKind::AggregateFunction => {
            let info = meta.lookup_function(text)?;
            let mut md = String::new();
            if !info.syntax.is_empty() {
                md.push_str(&format!("```\n{}\n```\n", info.syntax));
            } else {
                md.push_str(&format!("**{}**\n", info.name));
            }
            if !info.description.is_empty() {
                md.push_str(&format!("\n{}\n", info.description));
            }
            if !info.arguments.is_empty() {
                md.push_str(&format!("\n**Arguments:** {}\n", info.arguments));
            }
            if !info.returned_value.is_empty() {
                md.push_str(&format!("\n**Returns:** {}\n", info.returned_value));
            }
            if info.is_aggregate {
                md.push_str("\n*Aggregate function*\n");
            }
            md
        }

        SyntaxKind::SettingItem | SyntaxKind::SettingList | SyntaxKind::SettingsClause => {
            let info = meta
                .settings
                .iter()
                .chain(meta.merge_tree_settings.iter())
                .find(|s| s.name.eq_ignore_ascii_case(text))?;
            format!(
                "**{}** ({})\n\nDefault: `{}`\n\n{}",
                info.name, info.value_type, info.default, info.description
            )
        }

        SyntaxKind::DataType | SyntaxKind::DataTypeParameters => {
            let info = meta
                .data_types
                .iter()
                .find(|dt| dt.name.eq_ignore_ascii_case(text))?;
            if !info.alias_to.is_empty() {
                format!("**{}** — alias for **{}**", info.name, info.alias_to)
            } else {
                format!("**{}**", info.name)
            }
        }

        SyntaxKind::EngineClause => {
            let info = meta.table_engines.iter().find(|e| e.name == text)?;
            let mut md = format!("**{}** engine\n", info.name);
            let mut features = Vec::new();
            if info.supports_replication {
                features.push("replication");
            }
            if info.supports_sort_order {
                features.push("sort order");
            }
            if info.supports_ttl {
                features.push("TTL");
            }
            if info.supports_settings {
                features.push("settings");
            }
            if !features.is_empty() {
                md.push_str(&format!("\nSupports: {}", features.join(", ")));
            }
            md
        }

        // Table identifier — show column list if connected
        SyntaxKind::TableIdentifier | SyntaxKind::FromClause => {
            let default_db = meta.default_database().to_string();
            let scope = build_scope(&parse.tree, &parse.source);
            let (db, table) = resolve_table_for_hover(text, &scope, &default_db);
            let columns = meta.get_columns(&db, &table);
            if !columns.is_empty() {
                let mut md = format!("**{}.{}**\n\n", db, table);
                md.push_str("| Column | Type |\n|--------|------|\n");
                for col in columns {
                    md.push_str(&format!("| {} | `{}` |\n", col.name, col.data_type));
                }
                md
            } else {
                // No columns cached — try function lookup as fallback
                if let Some(info) = meta.lookup_function(text) {
                    hover_function(info)
                } else {
                    return None;
                }
            }
        }

        // For any bareword, try function lookup as fallback
        _ => {
            if let Some(info) = meta.lookup_function(text) {
                hover_function(info)
            } else {
                return None;
            }
        }
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: contents,
        }),
        range: Some(line_index.range(token_start, token_end)),
    })
}

fn hover_function(info: &crate::metadata::types::FunctionInfo) -> String {
    let mut md = String::new();
    if !info.syntax.is_empty() {
        md.push_str(&format!("```\n{}\n```\n", info.syntax));
    }
    if !info.description.is_empty() {
        md.push_str(&format!("\n{}\n", info.description));
    }
    md
}

/// Resolve a table name for hover — check if it's an alias first.
fn resolve_table_for_hover(
    text: &str,
    scope: &crate::analysis::scope::QueryScope,
    default_db: &str,
) -> (String, String) {
    // Check table refs directly
    for tref in &scope.table_refs {
        if tref.table.eq_ignore_ascii_case(text) {
            let db = tref
                .database
                .clone()
                .unwrap_or_else(|| default_db.to_string());
            return (db, tref.table.clone());
        }
    }
    (default_db.to_string(), text.to_string())
}

/// Find the non-trivia token at `offset` and return (text, parent_kind, start, end).
fn find_token_with_parent<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    offset: u32,
) -> Option<(&'a str, SyntaxKind, u32, u32)> {
    find_token_impl(tree, source, offset, tree.kind)
}

fn find_token_impl<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    offset: u32,
    parent_kind: SyntaxKind,
) -> Option<(&'a str, SyntaxKind, u32, u32)> {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                if token.kind == SyntaxKind::Whitespace || token.kind == SyntaxKind::Comment {
                    continue;
                }
                if token.start <= offset && offset <= token.end {
                    return Some((token.text(source), parent_kind, token.start, token.end));
                }
            }
            SyntaxChild::Tree(subtree) => {
                if subtree.start > subtree.end {
                    continue;
                }
                if subtree.start <= offset && offset <= subtree.end {
                    if let Some(result) = find_token_impl(subtree, source, offset, subtree.kind) {
                        return Some(result);
                    }
                }
            }
        }
    }
    None
}
