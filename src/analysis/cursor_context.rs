use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

/// What the user is typing at a given cursor position.
#[derive(Debug, Clone, PartialEq)]
pub enum CursorContext {
    /// In a SELECT clause — expecting an expression, column, or function
    SelectExpression,
    /// After FROM/JOIN — expecting a table reference
    TableReference { database_prefix: Option<String> },
    /// After `qualifier.` — expecting a column name
    ColumnOfTable { qualifier: String },
    /// In a general expression context (WHERE, HAVING, etc.)
    Expression,
    /// In SETTINGS clause — expecting a setting name
    SettingName,
    /// After ENGINE = — expecting an engine name
    EngineName,
    /// After FORMAT — expecting a format name
    FormatName,
    /// In a type position (column definitions, CAST, etc.)
    DataType,
    /// Inside CODEC() — expecting a codec name
    CodecName,
    /// Inside function call arguments
    FunctionArgument {
        function_name: String,
        argument_index: usize,
    },
    /// At statement start — expecting SELECT, CREATE, etc.
    StatementStart,
    /// After a clause keyword
    ClauseKeyword { clause: SyntaxKind },
    /// Unknown / not determinable
    Unknown,
}

/// Determine the cursor context at a given byte offset in the CST.
pub fn cursor_context(tree: &SyntaxTree, source: &str, offset: u32) -> CursorContext {
    let path = find_node_path(tree, offset);
    analyze_path(&path, source, offset)
}

/// A node in the path from root to cursor, with reference to its subtree.
struct PathNode<'a> {
    kind: SyntaxKind,
    tree: &'a SyntaxTree,
}

/// Walk the CST from root to the deepest node containing `offset`.
fn find_node_path(tree: &SyntaxTree, offset: u32) -> Vec<PathNode<'_>> {
    let mut path = vec![PathNode {
        kind: tree.kind,
        tree,
    }];
    let mut current = tree;
    loop {
        let mut found = false;
        for child in &current.children {
            if let SyntaxChild::Tree(subtree) = child {
                if subtree.start > subtree.end {
                    continue; // empty node
                }
                if subtree.start <= offset && offset <= subtree.end {
                    path.push(PathNode {
                        kind: subtree.kind,
                        tree: subtree,
                    });
                    current = subtree;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
    }
    path
}

/// Find the token just before (or at) the offset, returning its text and parent kind.
fn token_before_offset<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    offset: u32,
) -> Option<(&'a str, SyntaxKind)> {
    let mut best: Option<(&str, SyntaxKind)> = None;
    token_before_impl(tree, source, offset, tree.kind, &mut best);
    best
}

fn token_before_impl<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    offset: u32,
    parent_kind: SyntaxKind,
    best: &mut Option<(&'a str, SyntaxKind)>,
) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                if token.kind == SyntaxKind::Whitespace || token.kind == SyntaxKind::Comment {
                    continue;
                }
                if token.start <= offset {
                    *best = Some((token.text(source), parent_kind));
                }
            }
            SyntaxChild::Tree(subtree) => {
                if subtree.start > subtree.end {
                    continue;
                }
                if subtree.start <= offset {
                    token_before_impl(subtree, source, offset, subtree.kind, best);
                }
            }
        }
    }
}

fn analyze_path(path: &[PathNode<'_>], source: &str, offset: u32) -> CursorContext {
    if path.is_empty() {
        return CursorContext::Unknown;
    }

    // When the cursor is at the end of a statement (past the last token),
    // find_node_path may not descend into the last clause if its Error child
    // is empty. Check the last clause of the deepest statement node.
    if let Some(ctx) = check_trailing_clause(path, offset) {
        return ctx;
    }

    // Check ancestors from deepest to shallowest
    for (i, node) in path.iter().enumerate().rev() {
        // Skip Error nodes — look at their parent for context
        if node.kind == SyntaxKind::Error {
            continue;
        }

        match node.kind {
            // FROM clause → table reference
            SyntaxKind::FromClause => {
                return table_ref_or_dot(node.tree, source, offset);
            }

            // JOIN clause → table reference OR expression (after ON/USING)
            SyntaxKind::JoinClause => {
                // Check if we're past the ON keyword
                if let Some((prev_text, _)) = token_before_offset(node.tree, source, offset) {
                    if prev_text.eq_ignore_ascii_case("ON")
                        || prev_text.eq_ignore_ascii_case("USING")
                    {
                        return CursorContext::Expression;
                    }
                }
                // Check if we're inside a JoinConstraint child
                for pn in path.iter().rev() {
                    if pn.kind == SyntaxKind::JoinConstraint {
                        return CursorContext::Expression;
                    }
                }
                return table_ref_or_dot(node.tree, source, offset);
            }

            // INSERT statement → table reference after INTO
            SyntaxKind::InsertStatement => {
                // If we're directly inside the InsertStatement (not in a deeper clause),
                // and the cursor is after INTO, suggest tables
                if i == path.len() - 1 || path[i + 1].kind == SyntaxKind::TableIdentifier {
                    return table_ref_or_dot(node.tree, source, offset);
                }
            }

            // SETTINGS clause → setting name
            SyntaxKind::SettingsClause | SyntaxKind::SettingList => {
                // If the cursor is at a position before '=', it's a setting name
                return CursorContext::SettingName;
            }

            // ENGINE clause → engine name
            SyntaxKind::EngineClause => {
                return CursorContext::EngineName;
            }

            // FORMAT clause → format name
            SyntaxKind::FormatClause | SyntaxKind::InsertFormatClause => {
                return CursorContext::FormatName;
            }

            // Data type positions
            SyntaxKind::DataType
            | SyntaxKind::ColumnTypeDefinition
            | SyntaxKind::DataTypeParameters => {
                return CursorContext::DataType;
            }

            // Column definition — after the column name, expect a type
            SyntaxKind::ColumnDefinition => {
                // If we're directly in ColumnDefinition (not in a nested DataType etc.),
                // the cursor is after the column name, expecting a type
                if i == path.len() - 1 || path[i + 1].kind == SyntaxKind::Error {
                    return CursorContext::DataType;
                }
            }

            // Column codec
            SyntaxKind::ColumnCodec => {
                return CursorContext::CodecName;
            }

            // Function call → function argument
            SyntaxKind::FunctionCall | SyntaxKind::AggregateFunction => {
                let fn_name = extract_function_name(node.tree, source);
                let arg_index = count_commas_before(node.tree, offset);
                return CursorContext::FunctionArgument {
                    function_name: fn_name,
                    argument_index: arg_index,
                };
            }

            // SELECT clause → expression context
            SyntaxKind::SelectClause => {
                // Check for dot access (table.column)
                if let Some(ctx) = check_dot_access(node.tree, source, offset) {
                    return ctx;
                }
                return CursorContext::SelectExpression;
            }

            // Expression contexts
            SyntaxKind::WhereClause
            | SyntaxKind::HavingClause
            | SyntaxKind::PrewhereClause
            | SyntaxKind::GroupByClause
            | SyntaxKind::OrderByClause => {
                if let Some(ctx) = check_dot_access(node.tree, source, offset) {
                    return ctx;
                }
                return CursorContext::Expression;
            }

            // Statement nodes → if we're at the very start, it's a statement start
            SyntaxKind::File | SyntaxKind::QueryList => {
                if i == path.len() - 1 {
                    // Deepest node is the file/query list itself
                    return CursorContext::StatementStart;
                }
            }

            _ => {}
        }
    }

    // Fallback: check if we're at the start of a statement
    if let Some(root) = path.first() {
        if root.kind == SyntaxKind::File {
            // Check if offset is after a semicolon or at document start
            if let Some((prev_text, _)) = token_before_offset(root.tree, source, offset) {
                if prev_text == ";" {
                    return CursorContext::StatementStart;
                }
            } else {
                return CursorContext::StatementStart;
            }
        }
    }

    CursorContext::Unknown
}

/// When the cursor is past the end of the last clause in a statement (e.g., `HAVING |`
/// where the Error node is empty), check the last child clause of the deepest statement.
fn check_trailing_clause(path: &[PathNode<'_>], offset: u32) -> Option<CursorContext> {
    // Find the deepest statement-like node in the path
    let stmt = path.iter().rev().find(|n| matches!(n.kind,
        SyntaxKind::SelectStatement | SyntaxKind::InsertStatement |
        SyntaxKind::CreateStatement | SyntaxKind::AlterStatement |
        SyntaxKind::DeleteStatement | SyntaxKind::UpdateStatement
    ))?;

    // Find the last clause child whose start is before the cursor
    let mut last_clause = None;
    for child in &stmt.tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            if subtree.start <= offset {
                last_clause = Some(subtree);
            }
        }
    }

    let clause = last_clause?;
    // Only activate if the cursor is past the clause's end (i.e., the clause
    // didn't fully contain our cursor — we're in trailing whitespace)
    if offset <= clause.end {
        return None;
    }

    match clause.kind {
        SyntaxKind::HavingClause | SyntaxKind::WhereClause | SyntaxKind::PrewhereClause => {
            Some(CursorContext::Expression)
        }
        SyntaxKind::FromClause => {
            Some(CursorContext::TableReference { database_prefix: None })
        }
        SyntaxKind::SelectClause => {
            Some(CursorContext::SelectExpression)
        }
        SyntaxKind::GroupByClause | SyntaxKind::OrderByClause => {
            Some(CursorContext::Expression)
        }
        SyntaxKind::SettingsClause => {
            Some(CursorContext::SettingName)
        }
        SyntaxKind::FormatClause => {
            Some(CursorContext::FormatName)
        }
        _ => None,
    }
}

/// Return TableReference, with database_prefix if after a dot.
fn table_ref_or_dot(tree: &SyntaxTree, source: &str, offset: u32) -> CursorContext {
    if let Some((prev_text, _)) = token_before_offset(tree, source, offset) {
        if prev_text == "." {
            if let Some(db) = find_identifier_before_dot(tree, source, offset) {
                return CursorContext::TableReference {
                    database_prefix: Some(db),
                };
            }
        }
    }
    CursorContext::TableReference {
        database_prefix: None,
    }
}

/// Check if cursor is after a dot, indicating column access.
fn check_dot_access(tree: &SyntaxTree, source: &str, offset: u32) -> Option<CursorContext> {
    if let Some((prev_text, _parent)) = token_before_offset(tree, source, offset) {
        if prev_text == "." {
            if let Some(qualifier) = find_identifier_before_dot(tree, source, offset) {
                return Some(CursorContext::ColumnOfTable { qualifier });
            }
        }
    }
    None
}

/// Find the identifier text immediately before a dot at the given offset.
fn find_identifier_before_dot(
    tree: &SyntaxTree,
    source: &str,
    offset: u32,
) -> Option<String> {
    let mut prev_prev: Option<&str> = None;
    let mut prev: Option<&str> = None;
    find_ident_before_dot_impl(tree, source, offset, &mut prev_prev, &mut prev);
    // prev should be ".", prev_prev should be the identifier
    if prev == Some(".") {
        prev_prev.map(|s| s.to_string())
    } else {
        None
    }
}

fn find_ident_before_dot_impl<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    offset: u32,
    prev_prev: &mut Option<&'a str>,
    prev: &mut Option<&'a str>,
) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                if token.kind == SyntaxKind::Whitespace || token.kind == SyntaxKind::Comment {
                    continue;
                }
                if token.start > offset {
                    return;
                }
                *prev_prev = *prev;
                *prev = Some(token.text(source));
            }
            SyntaxChild::Tree(subtree) => {
                if subtree.start > subtree.end {
                    continue;
                }
                if subtree.start <= offset {
                    find_ident_before_dot_impl(subtree, source, offset, prev_prev, prev);
                }
            }
        }
    }
}

/// Extract function name from a FunctionCall node.
fn extract_function_name(tree: &SyntaxTree, source: &str) -> String {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) if token.kind == SyntaxKind::BareWord => {
                return token.text(source).to_string();
            }
            SyntaxChild::Tree(subtree) if subtree.kind == SyntaxKind::Identifier => {
                // Get the first bareword token in the identifier
                for sub_child in &subtree.children {
                    if let SyntaxChild::Token(token) = sub_child {
                        if token.kind == SyntaxKind::BareWord {
                            return token.text(source).to_string();
                        }
                    }
                }
            }
            _ => {}
        }
        // Stop at the opening paren — function name is always before it
        if let SyntaxChild::Token(token) = child {
            if token.kind == SyntaxKind::OpeningRoundBracket {
                break;
            }
        }
    }
    String::new()
}

/// Count commas before `offset` within a tree (for argument index).
fn count_commas_before(tree: &SyntaxTree, offset: u32) -> usize {
    let mut count = 0;
    let mut inside_parens = false;
    count_commas_impl(tree, offset, &mut count, &mut inside_parens);
    count
}

fn count_commas_impl(tree: &SyntaxTree, offset: u32, count: &mut usize, inside_parens: &mut bool) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                if token.start >= offset {
                    return;
                }
                if token.kind == SyntaxKind::OpeningRoundBracket {
                    *inside_parens = true;
                    *count = 0; // reset — we're entering the arg list
                } else if token.kind == SyntaxKind::Comma && *inside_parens {
                    *count += 1;
                }
            }
            SyntaxChild::Tree(subtree) => {
                if subtree.start > subtree.end {
                    continue;
                }
                // Don't recurse into nested function calls
                if subtree.kind == SyntaxKind::FunctionCall
                    || subtree.kind == SyntaxKind::AggregateFunction
                {
                    continue;
                }
                count_commas_impl(subtree, offset, count, inside_parens);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn ctx_at(sql: &str, cursor: usize) -> CursorContext {
        let parse = parser::parse(sql);
        cursor_context(&parse.tree, &parse.source, cursor as u32)
    }

    #[test]
    fn empty_file() {
        assert_eq!(ctx_at("", 0), CursorContext::StatementStart);
    }

    #[test]
    fn after_semicolon() {
        assert_eq!(ctx_at("SELECT 1;", 9), CursorContext::StatementStart);
    }

    #[test]
    fn from_clause_table() {
        assert_eq!(
            ctx_at("SELECT 1 FROM ", 14),
            CursorContext::TableReference {
                database_prefix: None
            }
        );
    }

    #[test]
    fn from_clause_with_database() {
        assert_eq!(
            ctx_at("SELECT 1 FROM db.", 17),
            CursorContext::TableReference {
                database_prefix: Some("db".into())
            }
        );
    }

    #[test]
    fn select_expression() {
        assert_eq!(ctx_at("SELECT ", 7), CursorContext::SelectExpression);
    }

    #[test]
    fn where_expression() {
        assert_eq!(
            ctx_at("SELECT 1 FROM t WHERE ", 22),
            CursorContext::Expression
        );
    }

    #[test]
    fn function_argument() {
        let ctx = ctx_at("SELECT toDateTime(", 18);
        assert!(matches!(
            ctx,
            CursorContext::FunctionArgument {
                ref function_name,
                argument_index: 0
            } if function_name == "toDateTime"
        ));
    }

    #[test]
    fn function_second_argument() {
        let ctx = ctx_at("SELECT toDateTime(x, ", 21);
        assert!(matches!(
            ctx,
            CursorContext::FunctionArgument {
                ref function_name,
                argument_index: 1
            } if function_name == "toDateTime"
        ));
    }

    #[test]
    fn settings_clause() {
        assert_eq!(
            ctx_at("SELECT 1 SETTINGS ", 18),
            CursorContext::SettingName
        );
    }

    #[test]
    fn column_after_dot() {
        assert_eq!(
            ctx_at("SELECT t. FROM t", 9),
            CursorContext::ColumnOfTable {
                qualifier: "t".into()
            }
        );
    }

    #[test]
    fn engine_clause() {
        assert_eq!(
            ctx_at("CREATE TABLE t (a Int32) ENGINE = ", 33),
            CursorContext::EngineName
        );
    }

    #[test]
    fn format_clause() {
        assert_eq!(
            ctx_at("SELECT 1 FORMAT ", 16),
            CursorContext::FormatName
        );
    }

    // --- Suspected gap tests ---

    #[test]
    fn group_by_expression() {
        assert_eq!(
            ctx_at("SELECT a FROM t GROUP BY ", 25),
            CursorContext::Expression
        );
    }

    #[test]
    fn order_by_expression() {
        assert_eq!(
            ctx_at("SELECT a FROM t ORDER BY ", 25),
            CursorContext::Expression
        );
    }

    #[test]
    fn having_expression() {
        let sql = "SELECT a, count() FROM t GROUP BY a HAVING ";
        assert_eq!(
            ctx_at(sql, sql.len()),
            CursorContext::Expression
        );
    }

    #[test]
    fn join_on_expression() {
        assert_eq!(
            ctx_at("SELECT a FROM t JOIN u ON ", 25),
            CursorContext::Expression
        );
    }

    #[test]
    fn insert_into_table() {
        assert_eq!(
            ctx_at("INSERT INTO ", 12),
            CursorContext::TableReference { database_prefix: None }
        );
    }

    #[test]
    fn after_comma_in_select() {
        assert_eq!(
            ctx_at("SELECT a, ", 10),
            CursorContext::SelectExpression
        );
    }

    #[test]
    fn create_table_column_type() {
        assert_eq!(
            ctx_at("CREATE TABLE t (col ", 20),
            CursorContext::DataType
        );
    }
}
