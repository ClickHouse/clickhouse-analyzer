use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

/// A named binding within a query (CTE, table alias, or column alias).
#[derive(Debug, Clone)]
pub struct NameBinding {
    pub name: String,
    /// Byte range of the name token itself.
    pub range: (u32, u32),
    /// Byte range of the full definition (e.g., CTE body, table expression).
    pub definition_range: (u32, u32),
}

/// A table reference in FROM/JOIN.
#[derive(Debug, Clone)]
pub struct TableRef {
    pub database: Option<String>,
    pub table: String,
    pub alias: Option<String>,
    pub range: (u32, u32),
}

/// A subquery used in a FROM position, together with its alias and the
/// list of column names produced by its top-level projection. Gives the
/// completion engine something to show when the user writes `alias.`.
#[derive(Debug, Clone)]
pub struct SubqueryRef {
    pub alias: String,
    pub columns: Vec<String>,
}

/// Resolved names within a single query.
#[derive(Debug, Clone, Default)]
pub struct QueryScope {
    pub ctes: Vec<NameBinding>,
    pub table_aliases: Vec<NameBinding>,
    pub column_aliases: Vec<NameBinding>,
    pub table_refs: Vec<TableRef>,
    pub subquery_refs: Vec<SubqueryRef>,
}

impl QueryScope {
    /// Columns produced by a subquery with the given alias, case-insensitive.
    pub fn subquery_columns_for(&self, alias: &str) -> Option<Vec<String>> {
        self.subquery_refs
            .iter()
            .find(|s| s.alias.eq_ignore_ascii_case(alias))
            .map(|s| s.columns.clone())
    }
}

/// Build a scope from the CST of a statement.
pub fn build_scope(tree: &SyntaxTree, source: &str) -> QueryScope {
    let mut scope = QueryScope::default();
    collect_scope(tree, source, &mut scope);
    scope
}

fn collect_scope(tree: &SyntaxTree, source: &str, scope: &mut QueryScope) {
    match tree.kind {
        SyntaxKind::WithClause => {
            collect_ctes(tree, source, scope);
            return; // Don't recurse further into WITH
        }
        SyntaxKind::FromClause | SyntaxKind::JoinClause => {
            collect_table_refs(tree, source, scope);
        }
        SyntaxKind::SelectClause => {
            collect_column_aliases(tree, source, scope);
        }
        _ => {}
    }

    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            // Don't recurse into subqueries — they have their own scope
            if subtree.kind == SyntaxKind::SubqueryExpression {
                continue;
            }
            collect_scope(subtree, source, scope);
        }
    }
}

/// Extract CTEs from a WITH clause.
/// CST: WithClause → ColumnList → WithExpressionItem
fn collect_ctes(tree: &SyntaxTree, source: &str, scope: &mut QueryScope) {
    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            if subtree.kind == SyntaxKind::WithExpressionItem {
                if let Some(binding) = extract_cte(subtree, source) {
                    scope.ctes.push(binding);
                }
            } else if subtree.kind == SyntaxKind::ColumnList {
                // Recurse into ColumnList
                collect_ctes(subtree, source, scope);
            }
        }
    }
}

/// Strip surrounding quotes (backticks or double-quotes) from an identifier.
fn unquote(s: &str) -> &str {
    if (s.starts_with('`') && s.ends_with('`'))
        || (s.starts_with('"') && s.ends_with('"'))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Extract CTE name from a WITH expression item.
/// CST pattern: `name 'AS' '(' SubqueryExpression ')'`
/// The name is the first BareWord or QuotedIdentifier before AS.
fn extract_cte(tree: &SyntaxTree, source: &str) -> Option<NameBinding> {
    for child in &tree.children {
        if let SyntaxChild::Token(token) = child {
            if token.kind == SyntaxKind::BareWord {
                let text = token.text(source);
                if !text.eq_ignore_ascii_case("AS") {
                    return Some(NameBinding {
                        name: text.to_string(),
                        range: (token.start, token.end),
                        definition_range: (tree.start, tree.end),
                    });
                }
            } else if token.kind == SyntaxKind::QuotedIdentifier {
                let text = unquote(token.text(source));
                return Some(NameBinding {
                    name: text.to_string(),
                    range: (token.start, token.end),
                    definition_range: (tree.start, tree.end),
                });
            }
        }
    }
    None
}

/// Extract table references and aliases from FROM/JOIN clauses.
/// CST structure: FromClause contains TableIdentifier and TableAlias as siblings.
fn collect_table_refs(tree: &SyntaxTree, source: &str, scope: &mut QueryScope) {
    let mut last_table_ref: Option<TableRef> = None;
    // Track the most recent subquery in the FROM position so a trailing
    // TableAlias can be attached to it as a SubqueryRef.
    let mut last_subquery_projection: Option<Vec<String>> = None;

    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            match subtree.kind {
                SyntaxKind::TableIdentifier => {
                    if let Some(tref) = last_table_ref.take() {
                        scope.table_refs.push(tref);
                    }
                    last_subquery_projection = None;
                    last_table_ref = extract_table_identifier(subtree, source);
                }
                SyntaxKind::TableAlias => {
                    if let Some((alias_name, alias_token)) =
                        extract_alias_name(subtree, source)
                    {
                        if let Some(ref mut tref) = last_table_ref {
                            tref.alias = Some(alias_name.clone());
                            scope.table_aliases.push(NameBinding {
                                name: alias_name,
                                range: (alias_token.start, alias_token.end),
                                definition_range: (subtree.start, subtree.end),
                            });
                        } else if let Some(cols) = last_subquery_projection.take() {
                            scope.table_aliases.push(NameBinding {
                                name: alias_name.clone(),
                                range: (alias_token.start, alias_token.end),
                                definition_range: (subtree.start, subtree.end),
                            });
                            scope.subquery_refs.push(SubqueryRef {
                                alias: alias_name,
                                columns: cols,
                            });
                        }
                    }
                }
                SyntaxKind::SubqueryExpression => {
                    // Record the subquery's projection so a following alias
                    // can claim it. The subquery's own FROM is intentionally
                    // NOT added to the outer scope.
                    last_subquery_projection =
                        Some(extract_subquery_projection(subtree, source));
                }
                SyntaxKind::TableExpression => {
                    collect_table_refs(subtree, source, scope);
                }
                SyntaxKind::JoinClause => {
                    if let Some(tref) = last_table_ref.take() {
                        scope.table_refs.push(tref);
                    }
                    last_subquery_projection = None;
                    collect_table_refs(subtree, source, scope);
                }
                _ => {}
            }
        }
    }

    if let Some(tref) = last_table_ref {
        scope.table_refs.push(tref);
    }
}

/// Extract the top-level projection column names from a SubqueryExpression.
/// Returns the AS-alias where present, otherwise the last segment of a
/// dotted column reference, otherwise a synthetic name like `col_1`.
fn extract_subquery_projection(tree: &SyntaxTree, source: &str) -> Vec<String> {
    let Some(select_clause) = find_child(tree, SyntaxKind::SelectClause)
        .or_else(|| find_descendant_select_clause(tree))
    else {
        return Vec::new();
    };
    let Some(column_list) = find_child(select_clause, SyntaxKind::ColumnList) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut pending_expr_name: Option<String> = None;
    let mut index = 0usize;

    for child in &column_list.children {
        match child {
            SyntaxChild::Tree(sub) => {
                if sub.kind == SyntaxKind::ColumnAlias {
                    if let Some((name, _)) = extract_alias_name(sub, source) {
                        out.push(name);
                        pending_expr_name = None;
                        index += 1;
                    }
                } else {
                    if let Some(prev) = pending_expr_name.take() {
                        out.push(prev);
                        index += 1;
                    }
                    pending_expr_name = Some(
                        last_identifier_in(sub, source)
                            .unwrap_or_else(|| format!("col_{}", index + 1)),
                    );
                }
            }
            SyntaxChild::Token(tok) => {
                if tok.kind == SyntaxKind::Comma {
                    if let Some(prev) = pending_expr_name.take() {
                        out.push(prev);
                        index += 1;
                    }
                }
            }
        }
    }
    if let Some(prev) = pending_expr_name {
        out.push(prev);
    }
    out
}

fn find_child<'a>(tree: &'a SyntaxTree, kind: SyntaxKind) -> Option<&'a SyntaxTree> {
    tree.children.iter().find_map(|c| match c {
        SyntaxChild::Tree(t) if t.kind == kind => Some(t),
        _ => None,
    })
}

fn find_descendant_select_clause(tree: &SyntaxTree) -> Option<&SyntaxTree> {
    for child in &tree.children {
        if let SyntaxChild::Tree(t) = child {
            if t.kind == SyntaxKind::SelectClause {
                return Some(t);
            }
            if let Some(found) = find_descendant_select_clause(t) {
                return Some(found);
            }
        }
    }
    None
}

/// Walk the rightmost identifier-bearing path of an expression subtree to
/// guess the column name it would produce (e.g. `t.col` → `col`).
fn last_identifier_in(tree: &SyntaxTree, source: &str) -> Option<String> {
    let mut last: Option<String> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(tok) => {
                if tok.kind == SyntaxKind::BareWord || tok.kind == SyntaxKind::QuotedIdentifier {
                    let text = tok.text(source);
                    if text.eq_ignore_ascii_case("AS") {
                        continue;
                    }
                    last = Some(unquote(text).to_string());
                }
            }
            SyntaxChild::Tree(sub) => {
                if let Some(inner) = last_identifier_in(sub, source) {
                    last = Some(inner);
                }
            }
        }
    }
    last
}

/// Extract the alias name from a TableAlias or ColumnAlias node.
/// Pattern: `['AS'] name`
fn extract_alias_name<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
) -> Option<(String, &'a crate::Token)> {
    let mut last_ident = None;
    for child in &tree.children {
        if let SyntaxChild::Token(token) = child {
            if token.kind == SyntaxKind::BareWord {
                let text = token.text(source);
                if !text.eq_ignore_ascii_case("AS") {
                    last_ident = Some((text.to_string(), token));
                }
            } else if token.kind == SyntaxKind::QuotedIdentifier {
                let text = unquote(token.text(source));
                last_ident = Some((text.to_string(), token));
            }
        }
    }
    last_ident
}

fn extract_table_identifier(tree: &SyntaxTree, source: &str) -> Option<TableRef> {
    let mut parts = Vec::new();
    for child in &tree.children {
        if let SyntaxChild::Token(token) = child {
            if token.kind == SyntaxKind::BareWord || token.kind == SyntaxKind::QuotedIdentifier {
                parts.push(token.text(source).to_string());
            }
        }
    }

    match parts.as_slice() {
        [table] => Some(TableRef {
            database: None,
            table: table.clone(),
            alias: None,
            range: (tree.start, tree.end),
        }),
        [database, table] => Some(TableRef {
            database: Some(database.clone()),
            table: table.clone(),
            alias: None,
            range: (tree.start, tree.end),
        }),
        _ => None,
    }
}

/// Extract column aliases from a SELECT clause and its children.
/// ColumnAlias nodes appear as siblings in ColumnList.
fn collect_column_aliases(tree: &SyntaxTree, source: &str, scope: &mut QueryScope) {
    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            match subtree.kind {
                SyntaxKind::ColumnAlias => {
                    if let Some((name, token)) = extract_alias_name(subtree, source) {
                        scope.column_aliases.push(NameBinding {
                            name,
                            range: (token.start, token.end),
                            definition_range: (subtree.start, subtree.end),
                        });
                    }
                }
                SyntaxKind::ColumnList | SyntaxKind::ExpressionList => {
                    collect_column_aliases(subtree, source, scope);
                }
                _ => {}
            }
        }
    }
}

/// Find the enclosing statement node for a given byte offset.
pub fn find_enclosing_statement(tree: &SyntaxTree, offset: u32) -> Option<&SyntaxTree> {
    if tree.start > tree.end {
        return None;
    }
    if offset < tree.start || offset > tree.end {
        return None;
    }

    // Check if this is a statement node
    let is_statement = matches!(
        tree.kind,
        SyntaxKind::SelectStatement
            | SyntaxKind::InsertStatement
            | SyntaxKind::CreateStatement
            | SyntaxKind::AlterStatement
            | SyntaxKind::DeleteStatement
            | SyntaxKind::UpdateStatement
            | SyntaxKind::DropStatement
            | SyntaxKind::ShowStatement
            | SyntaxKind::ExplainStatement
            | SyntaxKind::DescribeStatement
    );

    // Try to find a deeper statement first
    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            if let Some(found) = find_enclosing_statement(subtree, offset) {
                return Some(found);
            }
        }
    }

    if is_statement {
        Some(tree)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn cte_scope() {
        let sql = "WITH cte AS (SELECT 1) SELECT * FROM cte";
        let parse = parser::parse(sql);
        let scope = build_scope(&parse.tree, &parse.source);
        assert_eq!(scope.ctes.len(), 1);
        assert_eq!(scope.ctes[0].name, "cte");
    }

    #[test]
    fn table_alias_scope() {
        let sql = "SELECT t.a FROM my_table AS t";
        let parse = parser::parse(sql);
        let scope = build_scope(&parse.tree, &parse.source);
        assert_eq!(scope.table_aliases.len(), 1);
        assert_eq!(scope.table_aliases[0].name, "t");
        assert_eq!(scope.table_refs.len(), 1);
        assert_eq!(scope.table_refs[0].table, "my_table");
    }

    #[test]
    fn table_ref_with_database() {
        let sql = "SELECT 1 FROM mydb.mytable";
        let parse = parser::parse(sql);
        let scope = build_scope(&parse.tree, &parse.source);
        assert!(!scope.table_refs.is_empty());
        let tref = &scope.table_refs[0];
        assert_eq!(tref.database.as_deref(), Some("mydb"));
        assert_eq!(tref.table, "mytable");
    }

    #[test]
    fn column_alias_scope() {
        let sql = "SELECT a + b AS total FROM t";
        let parse = parser::parse(sql);
        let scope = build_scope(&parse.tree, &parse.source);
        assert_eq!(scope.column_aliases.len(), 1);
        assert_eq!(scope.column_aliases[0].name, "total");
    }
}
