use super::types::Diagnostic;
use crate::parser::syntax_tree::SyntaxTree;
use crate::parser::syntax_tree::SyntaxChild;
use crate::parser::syntax_kind::SyntaxKind;

/// Find the innermost node with a display name that contains the given byte range.
/// This walks down the tree and returns the deepest ancestor that has a display name.
fn find_enclosing_node(tree: &SyntaxTree, range: (usize, usize)) -> Option<SyntaxKind> {
    find_enclosing_impl(tree, range, None)
}

fn find_enclosing_impl(
    tree: &SyntaxTree,
    range: (usize, usize),
    current_best: Option<SyntaxKind>,
) -> Option<SyntaxKind> {
    // Empty trees have start=MAX, end=0; skip them
    if tree.start > tree.end {
        return current_best;
    }

    // Check if the range falls within this tree
    if range.0 < tree.start as usize || range.1 > tree.end as usize {
        return current_best;
    }

    // Update best if this node has a display name
    let best = if kind_display_name(tree.kind).is_some() {
        Some(tree.kind)
    } else {
        current_best
    };

    // Try to find a more specific child
    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            let result = find_enclosing_impl(subtree, range, best);
            if result != best {
                return result;
            }
        }
    }

    best
}

fn kind_display_name(kind: SyntaxKind) -> Option<&'static str> {
    match kind {
        SyntaxKind::SelectStatement => Some("SELECT statement"),
        SyntaxKind::InsertStatement => Some("INSERT statement"),
        SyntaxKind::CreateStatement => Some("CREATE statement"),
        SyntaxKind::AlterStatement => Some("ALTER statement"),
        SyntaxKind::DeleteStatement => Some("DELETE statement"),
        SyntaxKind::DropStatement => Some("DROP statement"),
        SyntaxKind::AttachStatement => Some("ATTACH statement"),
        SyntaxKind::DetachStatement => Some("DETACH statement"),
        SyntaxKind::ExchangeStatement => Some("EXCHANGE statement"),
        SyntaxKind::UndropStatement => Some("UNDROP statement"),
        SyntaxKind::BackupStatement => Some("BACKUP statement"),
        SyntaxKind::RestoreStatement => Some("RESTORE statement"),
        SyntaxKind::ShowStatement => Some("SHOW statement"),
        SyntaxKind::ExplainStatement => Some("EXPLAIN statement"),
        SyntaxKind::DescribeStatement => Some("DESCRIBE statement"),
        SyntaxKind::SystemStatement => Some("SYSTEM statement"),
        SyntaxKind::KillStatement => Some("KILL statement"),
        SyntaxKind::WhereClause => Some("WHERE clause"),
        SyntaxKind::FromClause => Some("FROM clause"),
        SyntaxKind::SelectClause => Some("SELECT clause"),
        SyntaxKind::GroupByClause => Some("GROUP BY clause"),
        SyntaxKind::OrderByClause => Some("ORDER BY clause"),
        SyntaxKind::HavingClause => Some("HAVING clause"),
        SyntaxKind::LimitClause => Some("LIMIT clause"),
        SyntaxKind::JoinClause => Some("JOIN clause"),
        SyntaxKind::WithClause => Some("WITH clause"),
        SyntaxKind::SettingsClause => Some("SETTINGS clause"),
        SyntaxKind::EngineClause => Some("ENGINE clause"),
        SyntaxKind::ColumnDefinitionList => Some("column definitions"),
        SyntaxKind::GrantStatement => Some("GRANT statement"),
        SyntaxKind::RevokeStatement => Some("REVOKE statement"),
        _ => None,
    }
}

pub fn enrich(diagnostics: &mut [Diagnostic], tree: &SyntaxTree) {
    for diag in diagnostics.iter_mut() {
        if !diag.message.starts_with("Unexpected token") {
            continue;
        }

        if let Some(kind) = find_enclosing_node(tree, diag.range) {
            if let Some(name) = kind_display_name(kind) {
                diag.message = format!("Unexpected token in {}", name);
            }
        }
    }
}
