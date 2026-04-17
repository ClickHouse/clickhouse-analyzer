use tower_lsp::lsp_types::*;

use crate::analysis::cursor_context::{cursor_context, CursorContext};
use crate::analysis::scope::{build_scope, find_enclosing_statement, QueryScope};
use crate::metadata::cache::SharedMetadata;
use crate::parser::diagnostic::Parse;

use super::line_index::LineIndex;

/// Build the scope for the statement enclosing the cursor. When the cursor is
/// inside a subquery, this gives us the *inner* scope so column completions
/// are resolved against the subquery's own FROM clause, not the outer query.
fn scope_at(parse: &Parse, offset: u32) -> QueryScope {
    let stmt = find_enclosing_statement(&parse.tree, offset).unwrap_or(&parse.tree);
    build_scope(stmt, &parse.source)
}

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
                let scope = scope_at(parse, offset);
                // Subquery aliases resolve to their own projection — no
                // server lookup needed. Only pre-fetch for real tables.
                if scope.subquery_columns_for(qualifier).is_none() {
                    let default_db = meta.default_database().to_string();
                    if let Some((db, table)) = resolve_qualifier(qualifier, &scope, &default_db) {
                        let _ = meta.ensure_columns(&db, &table).await;
                    }
                }
            }
        }
        CursorContext::SelectExpression | CursorContext::Expression | CursorContext::FunctionArgument { .. } => {
            // Pre-fetch columns for all tables in scope
            let mut meta = metadata.write().await;
            if meta.is_connected() {
                let scope = scope_at(parse, offset);
                let default_db = meta.default_database().to_string();
                for tref in &scope.table_refs {
                    let db = tref.database.as_deref().unwrap_or(&default_db);
                    let _ = meta.ensure_columns(db, &tref.table).await;
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
            let scope = scope_at(parse, offset);
            // Subquery alias: expose the subquery's own projection directly.
            if let Some(cols) = scope.subquery_columns_for(qualifier) {
                for name in cols {
                    items.push(CompletionItem {
                        label: name,
                        kind: Some(CompletionItemKind::FIELD),
                        detail: Some("subquery projection".into()),
                        ..Default::default()
                    });
                }
            } else {
                // Regular table or table alias — resolve and look up metadata.
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
        }

        CursorContext::SelectExpression => {
            add_columns_in_scope(&meta, parse, offset, &mut items);
            add_functions(&meta.functions, &mut items);
            for kw in &["DISTINCT", "CASE", "NOT", "NULL", "TRUE", "FALSE", "*"] {
                items.push(keyword_item(kw));
            }
            // Clause keywords that can follow a select expression
            for kw in &[
                "FROM", "WHERE", "GROUP BY", "ORDER BY", "HAVING", "LIMIT",
                "FORMAT", "SETTINGS", "INTO OUTFILE", "UNION ALL", "EXCEPT",
                "INTERSECT", "AS",
            ] {
                items.push(keyword_item(kw));
            }
        }

        CursorContext::Expression => {
            add_columns_in_scope(&meta, parse, offset, &mut items);
            add_functions(&meta.functions, &mut items);
            for kw in &[
                "AND", "OR", "NOT", "IN", "BETWEEN", "LIKE", "ILIKE", "IS",
                "NULL", "TRUE", "FALSE", "CASE", "EXISTS",
            ] {
                items.push(keyword_item(kw));
            }
            // Clause keywords that can follow an expression
            for kw in &[
                "GROUP BY", "ORDER BY", "HAVING", "LIMIT",
                "FORMAT", "SETTINGS", "UNION ALL",
            ] {
                items.push(keyword_item(kw));
            }
        }

        CursorContext::FunctionArgument { .. } => {
            // Only show completions if the user has started typing a prefix
            // Otherwise signature help is more useful here
            if !lower_prefix.is_empty() {
                add_columns_in_scope(&meta, parse, offset, &mut items);
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

    // Assign sort order by item kind (keywords first, then fields, functions, etc.)
    // Case-insensitive — SQL users often type in lowercase
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
        item.sort_text = Some(format!("{}{}", kind_priority, item.label.to_lowercase()));
        // Also tell VS Code to match case-insensitively
        item.filter_text = Some(item.label.to_lowercase());
    }

    items
}

/// Add columns from all tables referenced in the query scope.
fn add_columns_in_scope(
    meta: &crate::metadata::cache::MetadataCache,
    parse: &Parse,
    offset: u32,
    items: &mut Vec<CompletionItem>,
) {
    if !meta.is_connected() {
        return;
    }
    let scope = scope_at(parse, offset);
    let default_db = meta.default_database().to_string();
    let mut seen = std::collections::HashSet::new();

    for tref in &scope.table_refs {
        let db = tref.database.as_deref().unwrap_or(&default_db);
        for col in meta.get_columns(db, &tref.table) {
            // Avoid duplicate column names from multiple tables
            if seen.insert(col.name.clone()) {
                let detail = if scope.table_refs.len() > 1 {
                    // Show which table the column is from when there are joins
                    format!("{} ({})", col.data_type, tref.table)
                } else {
                    col.data_type.clone()
                };
                items.push(CompletionItem {
                    label: col.name.clone(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(detail),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::scope::build_scope;
    use crate::parser;

    fn scope_for(sql: &str) -> crate::analysis::scope::QueryScope {
        let parse = parser::parse(sql);
        build_scope(&parse.tree, &parse.source)
    }

    #[test]
    fn resolve_table_alias() {
        let scope = scope_for("SELECT t.a FROM mytable AS t");
        let resolved = resolve_qualifier("t", &scope, "default");
        assert_eq!(resolved, Some(("default".into(), "mytable".into())));
    }

    #[test]
    fn resolve_table_alias_case_insensitive() {
        let scope = scope_for("SELECT T.a FROM mytable AS t");
        let resolved = resolve_qualifier("T", &scope, "default");
        assert_eq!(resolved, Some(("default".into(), "mytable".into())));
    }

    #[test]
    fn resolve_table_name() {
        let scope = scope_for("SELECT mytable.a FROM mytable");
        let resolved = resolve_qualifier("mytable", &scope, "default");
        assert_eq!(resolved, Some(("default".into(), "mytable".into())));
    }

    #[test]
    fn resolve_qualified_table_via_last_segment() {
        // `SELECT db.t.col FROM db.t` — the qualifier extracted at the
        // cursor is the last segment before the dot, which is `t`.
        // It should resolve to the fully-qualified ref.
        let scope = scope_for("SELECT db.t.a FROM db.t");
        let resolved = resolve_qualifier("t", &scope, "default");
        assert_eq!(resolved, Some(("db".into(), "t".into())));
    }

    #[test]
    fn resolve_alias_on_qualified_table() {
        let scope = scope_for("SELECT a.col FROM db.mytable AS a");
        let resolved = resolve_qualifier("a", &scope, "default");
        assert_eq!(resolved, Some(("db".into(), "mytable".into())));
    }

    #[test]
    fn resolve_in_join() {
        let scope = scope_for(
            "SELECT a.x, b.y FROM users AS a JOIN orders AS b ON a.id = b.user_id",
        );
        assert_eq!(
            resolve_qualifier("a", &scope, "default"),
            Some(("default".into(), "users".into()))
        );
        assert_eq!(
            resolve_qualifier("b", &scope, "default"),
            Some(("default".into(), "orders".into()))
        );
    }

    /// End-to-end: given a SQL string and cursor offset, return the resolved
    /// (database, table) pair the completion engine would look up columns in.
    fn resolve_at(sql: &str, cursor: usize) -> Option<(String, String)> {
        use crate::analysis::cursor_context::{cursor_context, CursorContext};
        let parse = parser::parse(sql);
        let scope = scope_at(&parse, cursor as u32);
        let ctx = cursor_context(&parse.tree, &parse.source, cursor as u32);
        match ctx {
            CursorContext::ColumnOfTable { qualifier } => {
                resolve_qualifier(&qualifier, &scope, "default")
            }
            _ => None,
        }
    }

    #[test]
    fn e2e_alias_after_dot_resolves() {
        // Cursor at `|` after `t.` in SELECT.
        let sql = "SELECT t. FROM mytable AS t";
        let cursor = sql.find('.').unwrap() + 1;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("default".into(), "mytable".into()))
        );
    }

    #[test]
    fn e2e_qualified_table_after_dot_resolves() {
        // Cursor at `|` after `t.` (last dot) in `SELECT db.t.| FROM db.t`.
        let sql = "SELECT db.t. FROM db.t";
        let cursor = sql.rfind("t. FROM").unwrap() + 2;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("db".into(), "t".into()))
        );
    }

    #[test]
    fn e2e_alias_resolves_when_select_is_malformed() {
        // User is mid-edit: SELECT clause has extra junk, FROM still parseable.
        // The resolver should still see the alias and resolve to the table.
        let sql = "SELECT x, a. FROM mytable AS a";
        let cursor = sql.find("a. FROM").unwrap() + 2;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("default".into(), "mytable".into())),
            "scope should still find alias 'a' when SELECT is mid-edit"
        );
    }

    #[test]
    fn e2e_qualified_table_in_where_resolves() {
        let sql = "SELECT * FROM db.mytable WHERE mytable. = 1";
        let cursor = sql.find("mytable. =").unwrap() + "mytable.".len();
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("db".into(), "mytable".into()))
        );
    }

    #[test]
    fn e2e_dot_at_end_of_partial_query_resolves() {
        // Cursor right at the end of a partial query. No trailing token at all.
        let sql = "SELECT t. FROM mytable AS t";
        let cursor = sql.find('.').unwrap() + 1;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("default".into(), "mytable".into()))
        );
    }

    #[test]
    fn e2e_dot_cursor_with_no_space_before_from() {
        // Live typing: user just hit `.` and autocomplete fires before any
        // trailing character. The adjacent `FROM` must not be consumed into
        // the qualified column reference.
        let sql = "SELECT t.FROM mytable AS t";
        let cursor = sql.find('.').unwrap() + 1;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("default".into(), "mytable".into())),
        );
    }

    #[test]
    fn e2e_cursor_inside_subquery_uses_inner_scope() {
        // Cursor is inside a subquery; the inner `FROM inner_tbl` must be
        // what defines the scope, not the outer query.
        let sql = "SELECT * FROM (SELECT x. FROM inner_tbl AS x) outer_alias";
        let cursor = sql.find("x. ").unwrap() + 2;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("default".into(), "inner_tbl".into())),
        );
    }

    #[test]
    fn e2e_subquery_alias_exposes_projection() {
        // Outer query references a subquery via its alias; the subquery's
        // projected columns (`a`, `b`) should be resolvable through the alias.
        let sql = "SELECT x. FROM (SELECT a, b FROM inner_tbl) x";
        let parse = parser::parse(sql);
        let scope = scope_at(&parse, (sql.find("x. ").unwrap() + 2) as u32);
        // The subquery alias `x` should expose `a` and `b` as columns.
        let cols = scope.subquery_columns_for("x");
        assert_eq!(
            cols,
            Some(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn e2e_subquery_alias_exposes_as_aliased_projection() {
        // Inner columns via expression + AS alias — should surface the alias.
        let sql = "SELECT x. FROM (SELECT count() AS total, user_id FROM t GROUP BY user_id) x";
        let parse = parser::parse(sql);
        let scope = scope_at(&parse, (sql.find("x. ").unwrap() + 2) as u32);
        let cols = scope.subquery_columns_for("x");
        assert_eq!(
            cols,
            Some(vec!["total".to_string(), "user_id".to_string()])
        );
    }

    #[test]
    fn e2e_subquery_alias_does_not_match_regular_table() {
        // Regular alias (not a subquery) returns None for subquery_columns_for.
        let sql = "SELECT x. FROM mytable AS x";
        let parse = parser::parse(sql);
        let scope = scope_at(&parse, (sql.find("x. ").unwrap() + 2) as u32);
        assert!(scope.subquery_columns_for("x").is_none());
    }

    #[test]
    fn e2e_alias_dot_works_across_multiple_columns() {
        // Real-world: cursor on `b.` after some columns already typed.
        let sql = "SELECT a.x, a.y, b. FROM users AS a JOIN orders AS b ON a.id = b.id";
        let cursor = sql.find("b. ").unwrap() + 2;
        assert_eq!(
            resolve_at(sql, cursor),
            Some(("default".into(), "orders".into()))
        );
    }
}
