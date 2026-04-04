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

/// Resolved names within a single query.
#[derive(Debug, Clone, Default)]
pub struct QueryScope {
    pub ctes: Vec<NameBinding>,
    pub table_aliases: Vec<NameBinding>,
    pub column_aliases: Vec<NameBinding>,
    pub table_refs: Vec<TableRef>,
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

/// Extract CTE name from a WITH expression item.
/// CST pattern: `name 'AS' '(' SubqueryExpression ')'`
/// The name is the first BareWord before AS.
fn extract_cte(tree: &SyntaxTree, source: &str) -> Option<NameBinding> {
    // The name is the first BareWord child (before AS)
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
            }
        }
    }
    None
}

/// Extract table references and aliases from FROM/JOIN clauses.
/// CST structure: FromClause contains TableIdentifier and TableAlias as siblings.
fn collect_table_refs(tree: &SyntaxTree, source: &str, scope: &mut QueryScope) {
    let mut last_table_ref: Option<TableRef> = None;

    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            match subtree.kind {
                SyntaxKind::TableIdentifier => {
                    // Flush previous table ref before starting a new one
                    if let Some(tref) = last_table_ref.take() {
                        scope.table_refs.push(tref);
                    }
                    last_table_ref = extract_table_identifier(subtree, source);
                }
                SyntaxKind::TableAlias => {
                    // Attach alias to the most recent table ref
                    if let Some(ref mut tref) = last_table_ref {
                        if let Some((alias_name, alias_token)) =
                            extract_alias_name(subtree, source)
                        {
                            tref.alias = Some(alias_name.clone());
                            scope.table_aliases.push(NameBinding {
                                name: alias_name,
                                range: (alias_token.start, alias_token.end),
                                definition_range: (subtree.start, subtree.end),
                            });
                        }
                    }
                }
                SyntaxKind::TableExpression => {
                    // Recurse into table expressions
                    collect_table_refs(subtree, source, scope);
                }
                SyntaxKind::JoinClause => {
                    // Flush before recursing into JOIN
                    if let Some(tref) = last_table_ref.take() {
                        scope.table_refs.push(tref);
                    }
                    collect_table_refs(subtree, source, scope);
                }
                _ => {}
            }
        }
    }

    // Flush the last table ref
    if let Some(tref) = last_table_ref {
        scope.table_refs.push(tref);
    }
}

/// Extract the alias name from a TableAlias or ColumnAlias node.
/// Pattern: `['AS'] name`
fn extract_alias_name<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
) -> Option<(String, &'a crate::Token)> {
    let mut last_bareword = None;
    for child in &tree.children {
        if let SyntaxChild::Token(token) = child {
            if token.kind == SyntaxKind::BareWord {
                let text = token.text(source);
                if !text.eq_ignore_ascii_case("AS") {
                    last_bareword = Some((text.to_string(), token));
                }
            }
        }
    }
    last_bareword
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
