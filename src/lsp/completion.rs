use tower_lsp::lsp_types::*;

use crate::analysis::cursor_context::{cursor_context, CursorContext};
use crate::analysis::scope::build_scope;
use crate::metadata::cache::SharedMetadata;
use crate::parser::diagnostic::Parse;

use super::line_index::LineIndex;

pub async fn handle_completion(
    parse: &Parse,
    line_index: &LineIndex,
    position: Position,
    metadata: &SharedMetadata,
) -> Vec<CompletionItem> {
    let offset = line_index.offset(position);
    let ctx = cursor_context(&parse.tree, &parse.source, offset);
    let prefix = extract_prefix(&parse.source, offset as usize);
    let lower_prefix = prefix.to_lowercase();

    // Pre-fetch schema data if connected and context requires it.
    // This needs a write lock to lazy-load tables/columns.
    match &ctx {
        CursorContext::TableReference { database_prefix } => {
            let mut meta = metadata.write().await;
            if meta.is_connected() {
                let db = database_prefix
                    .as_deref()
                    .unwrap_or(meta.default_database());
                let db = db.to_string();
                let _ = meta.ensure_tables(&db).await;
            }
        }
        CursorContext::ColumnOfTable { qualifier } => {
            let mut meta = metadata.write().await;
            if meta.is_connected() {
                // Resolve qualifier to a table name via scope
                let scope = build_scope(&parse.tree, &parse.source);
                let default_db = meta.default_database().to_string();
                if let Some((db, table)) = resolve_qualifier(qualifier, &scope, &default_db) {
                    let _ = meta.ensure_columns(&db, &table).await;
                }
            }
        }
        _ => {}
    }

    let meta = metadata.read().await;
    let mut items = Vec::new();

    match ctx {
        CursorContext::TableReference { database_prefix } => {
            if let Some(db) = database_prefix {
                // Complete table names in the given database
                for t in meta.get_tables(&db) {
                    items.push(CompletionItem {
                        label: t.name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!("{} ({})", t.engine, t.database)),
                        documentation: if t.comment.is_empty() {
                            None
                        } else {
                            Some(Documentation::String(t.comment.clone()))
                        },
                        ..Default::default()
                    });
                }
            } else {
                // Complete database names
                for db in &meta.databases {
                    items.push(CompletionItem {
                        label: db.name.clone(),
                        kind: Some(CompletionItemKind::MODULE),
                        detail: Some(db.engine.clone()),
                        ..Default::default()
                    });
                }
                // Tables in default database
                let default_db = meta.default_database().to_string();
                for t in meta.get_tables(&default_db) {
                    items.push(CompletionItem {
                        label: t.name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(t.engine.clone()),
                        ..Default::default()
                    });
                }
            }
        }

        CursorContext::ColumnOfTable { ref qualifier } => {
            // Resolve qualifier to database.table via scope, then show columns
            let scope = build_scope(&parse.tree, &parse.source);
            let default_db = meta.default_database().to_string();
            if let Some((db, table)) = resolve_qualifier(qualifier, &scope, &default_db) {
                for col in meta.get_columns(&db, &table) {
                    items.push(CompletionItem {
                        label: col.name.clone(),
                        kind: Some(CompletionItemKind::FIELD),
                        detail: Some(col.data_type.clone()),
                        documentation: if col.comment.is_empty() {
                            None
                        } else {
                            Some(Documentation::String(col.comment.clone()))
                        },
                        ..Default::default()
                    });
                }
            }
        }

        CursorContext::SelectExpression | CursorContext::Expression => {
            // Functions
            add_functions(&meta.functions, &mut items);
            // Keywords useful in expression context
            for kw in &[
                "AND", "OR", "NOT", "IN", "BETWEEN", "LIKE", "ILIKE", "IS", "NULL", "TRUE",
                "FALSE", "CASE", "WHEN", "THEN", "ELSE", "END", "AS", "DISTINCT",
            ] {
                items.push(keyword_item(kw));
            }
        }

        CursorContext::FunctionArgument { .. } => {
            // Only show completions if the user has started typing a prefix
            // Otherwise signature help is more useful here
            if !lower_prefix.is_empty() {
                add_functions(&meta.functions, &mut items);
            }
        }

        CursorContext::SettingName => {
            for s in &meta.settings {
                items.push(CompletionItem {
                    label: s.name.clone(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(format!("{} (default: {})", s.value_type, s.default)),
                    documentation: Some(Documentation::String(s.description.clone())),
                    ..Default::default()
                });
            }
            for s in &meta.merge_tree_settings {
                items.push(CompletionItem {
                    label: s.name.clone(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(format!("{} (default: {})", s.value_type, s.default)),
                    documentation: Some(Documentation::String(s.description.clone())),
                    ..Default::default()
                });
            }
        }

        CursorContext::EngineName => {
            for e in &meta.table_engines {
                items.push(CompletionItem {
                    label: e.name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    ..Default::default()
                });
            }
        }

        CursorContext::FormatName => {
            for f in &meta.formats {
                items.push(CompletionItem {
                    label: f.name.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(format!(
                        "{}{}",
                        if f.is_input { "input" } else { "" },
                        if f.is_output {
                            if f.is_input {
                                "/output"
                            } else {
                                "output"
                            }
                        } else {
                            ""
                        }
                    )),
                    ..Default::default()
                });
            }
        }

        CursorContext::DataType => {
            for dt in &meta.data_types {
                let detail = if dt.alias_to.is_empty() {
                    None
                } else {
                    Some(format!("alias for {}", dt.alias_to))
                };
                items.push(CompletionItem {
                    label: dt.name.clone(),
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
                    detail,
                    ..Default::default()
                });
            }
        }

        CursorContext::CodecName => {
            for c in &meta.codecs {
                items.push(CompletionItem {
                    label: c.name.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    ..Default::default()
                });
            }
        }

        CursorContext::StatementStart => {
            for kw in &[
                "SELECT", "INSERT INTO", "CREATE TABLE", "CREATE VIEW",
                "CREATE MATERIALIZED VIEW", "CREATE DATABASE", "ALTER TABLE", "DROP TABLE",
                "DROP DATABASE", "SHOW TABLES", "SHOW DATABASES", "SHOW CREATE TABLE",
                "DESCRIBE", "EXPLAIN", "USE", "SET", "OPTIMIZE TABLE", "SYSTEM",
                "TRUNCATE TABLE", "RENAME TABLE", "DELETE FROM", "UPDATE", "GRANT",
                "REVOKE", "WITH",
            ] {
                items.push(keyword_item(kw));
            }
        }

        CursorContext::ClauseKeyword { .. } | CursorContext::Unknown => {
            // General SQL keywords
            for kw in &[
                "SELECT", "FROM", "WHERE", "GROUP BY", "ORDER BY", "HAVING", "LIMIT",
                "JOIN", "LEFT JOIN", "RIGHT JOIN", "INNER JOIN", "CROSS JOIN",
                "ON", "USING", "AS", "UNION ALL", "EXCEPT", "INTERSECT",
                "INSERT INTO", "VALUES", "FORMAT", "SETTINGS", "PREWHERE",
                "SAMPLE", "ARRAY JOIN", "FINAL", "WITH",
            ] {
                items.push(keyword_item(kw));
            }
        }

    }

    // Prefix filter (case-insensitive)
    if !lower_prefix.is_empty() {
        items.retain(|item| item.label.to_lowercase().starts_with(&lower_prefix));
    }

    // Assign sort order: exact prefix matches rank higher,
    // then by item kind (keywords first, then fields, functions, etc.)
    for item in &mut items {
        let kind_priority = match item.kind {
            Some(CompletionItemKind::KEYWORD) => "0",
            Some(CompletionItemKind::FIELD) => "1",    // columns
            Some(CompletionItemKind::CLASS) => "2",    // tables/engines
            Some(CompletionItemKind::MODULE) => "2",   // databases
            Some(CompletionItemKind::FUNCTION) => "3",
            Some(CompletionItemKind::METHOD) => "3",   // aggregate functions
            Some(CompletionItemKind::PROPERTY) => "4", // settings
            _ => "5",
        };
        // Exact case match ranks above case-insensitive match
        let case_priority = if !lower_prefix.is_empty()
            && item.label.starts_with(prefix)
        {
            "0"
        } else {
            "1"
        };
        item.sort_text = Some(format!("{}{}{}", case_priority, kind_priority, item.label));
    }

    items
}

fn add_functions(
    functions: &[crate::metadata::types::FunctionInfo],
    items: &mut Vec<CompletionItem>,
) {
    for f in functions {
        if !f.alias_to.is_empty() {
            continue; // Skip aliases, show only canonical names
        }
        if f.name.starts_with("__") {
            continue; // Skip internal/undocumented functions
        }
        let kind = if f.is_aggregate {
            CompletionItemKind::METHOD
        } else {
            CompletionItemKind::FUNCTION
        };
        let detail = if f.syntax.is_empty() {
            None
        } else {
            Some(f.syntax.clone())
        };
        items.push(CompletionItem {
            label: f.name.clone(),
            kind: Some(kind),
            detail,
            insert_text: Some(format!("{}($0)", f.name)),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn keyword_item(kw: &str) -> CompletionItem {
    CompletionItem {
        label: kw.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        ..Default::default()
    }
}

/// Extract the word prefix being typed at `offset`.
fn extract_prefix(source: &str, offset: usize) -> &str {
    let bytes = source.as_bytes();
    let mut start = offset;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    &source[start..offset]
}

/// Resolve a qualifier (e.g., table alias or table name) to (database, table).
fn resolve_qualifier(
    qualifier: &str,
    scope: &crate::analysis::scope::QueryScope,
    default_db: &str,
) -> Option<(String, String)> {
    // Check if the qualifier matches a table alias
    for alias in &scope.table_aliases {
        if alias.name.eq_ignore_ascii_case(qualifier) {
            // Find the table ref this alias belongs to
            for tref in &scope.table_refs {
                if tref.alias.as_deref() == Some(&alias.name) {
                    let db = tref.database.clone().unwrap_or_else(|| default_db.to_string());
                    return Some((db, tref.table.clone()));
                }
            }
        }
    }

    // Check if the qualifier matches a table name directly
    for tref in &scope.table_refs {
        if tref.table.eq_ignore_ascii_case(qualifier) {
            let db = tref.database.clone().unwrap_or_else(|| default_db.to_string());
            return Some((db, tref.table.clone()));
        }
    }

    // Fallback: treat qualifier as a table name in the default database
    Some((default_db.to_string(), qualifier.to_string()))
}
