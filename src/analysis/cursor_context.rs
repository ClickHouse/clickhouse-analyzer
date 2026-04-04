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

    // Check ancestors from deepest to shallowest
    for (i, node) in path.iter().enumerate().rev() {
        match node.kind {
            // FROM / JOIN clause → table reference
            SyntaxKind::FromClause | SyntaxKind::JoinClause => {
                // Check if we're after a dot (database.table)
                if let Some((prev_text, _)) = token_before_offset(node.tree, source, offset) {
                    if prev_text == "." {
                        // Find the identifier before the dot
                        if let Some(db) = find_identifier_before_dot(node.tree, source, offset) {
                            return CursorContext::TableReference {
                                database_prefix: Some(db),
                            };
                        }
                    }
                }
                return CursorContext::TableReference {
                    database_prefix: None,
                };
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
}
