use super::context::FormatterContext;
use crate::lexer::token::{Token, TokenKind};
use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

// ---------------------------------------------------------------------------
// Keyword detection
// ---------------------------------------------------------------------------

/// All keyword uppercase forms for matching BareWord tokens.
const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "ORDER", "BY", "GROUP", "HAVING", "LIMIT",
    "OFFSET", "WITH", "AS", "ON", "USING", "BETWEEN", "IN", "LIKE", "ILIKE",
    "IS", "NOT", "CASE", "WHEN", "THEN", "ELSE", "END", "CAST", "DISTINCT",
    "ALL", "EXISTS", "AND", "OR", "JOIN", "INNER", "LEFT", "RIGHT", "FULL",
    "OUTER", "CROSS", "GLOBAL", "ANY", "SEMI", "ANTI", "ASOF", "NATURAL",
    "ARRAY", "FINAL", "ASC", "DESC", "NULLS", "FIRST", "LAST", "TOTALS",
    "ROLLUP", "CUBE", "UNION", "EXCEPT", "INTERSECT", "INSERT", "INTO",
    "VALUES", "DELETE", "UPDATE", "SET", "CREATE", "ALTER", "DROP", "DETACH",
    "ATTACH", "RENAME", "TRUNCATE", "SHOW", "USE", "OPTIMIZE", "SYSTEM",
    "EXCHANGE", "UNDROP", "TABLE", "VIEW", "DATABASE", "DICTIONARY",
    "FUNCTION", "MATERIALIZED", "TEMPORARY", "IF", "REPLACE", "LIVE",
    "DEFAULT", "CODEC", "TTL", "COMMENT", "PRIMARY", "KEY", "ALIAS",
    "EPHEMERAL", "PREWHERE", "SETTINGS", "FORMAT", "SAMPLE", "NULL", "TRUE",
    "FALSE", "INTERVAL", "ENGINE", "PARTITION", "CLUSTER", "TO", "POPULATE",
    "EMPTY", "PERMANENTLY", "AFTER", "COLUMN", "INDEX", "PROJECTION",
    "CONSTRAINT", "ADD", "MODIFY", "CLEAR", "MOVE", "GRANULARITY", "TYPE",
    "DEDUPLICATE", "EXPLAIN", "DESCRIBE", "AST", "PLAN", "PIPELINE",
    "ESTIMATE", "TABLES", "DATABASES", "COLUMNS", "DICTIONARIES",
    "FUNCTIONS", "PROCESSLIST", "PRIVILEGES", "GRANTS", "RELOAD", "FLUSH",
    "STOP", "START", "MERGES", "REPLICA", "REPLICAS", "DISTRIBUTED",
    "SENDING", "FETCHES", "MOVES", "LOGS", "CACHE", "DNS", "MARK",
    "UNCOMPRESSED", "COMPILED", "MODELS", "DISKS", "GRANT", "REVOKE",
    "USER", "ROLE", "QUOTA", "POLICY", "PROFILE", "ROW", "NONE", "KILL",
    "QUERY", "MUTATION", "SYNC", "ASYNC", "TEST", "CHECK", "BEGIN", "COMMIT",
    "ROLLBACK", "TRANSACTION", "BACKUP", "RESTORE", "LOCAL", "FREEZE",
    "UNFREEZE", "FETCH", "APPLY", "DELETED", "SOURCE", "LAYOUT", "LIFETIME",
    "RANGE", "HASHED", "FLAT", "COMPLEX", "DIRECT", "INJECTIVE",
    "HIERARCHICAL", "WINDOW", "OVER", "ROWS", "GROUPS", "UNBOUNDED",
    "PRECEDING", "FOLLOWING", "CURRENT", "DIV", "MOD", "DESC",
    "SYNTAX", "TREE", "OVERRIDE", "ENGINES", "FOR", "PART",
    "MATERIALIZE", "SETTING", "RESET", "ILIKE",
];

fn is_keyword(text: &str) -> bool {
    let upper = text.to_uppercase();
    KEYWORDS.contains(&upper.as_str())
}

/// Keywords that should NOT be uppercased (they act as literal values).
fn is_value_keyword(text: &str) -> bool {
    let upper = text.to_uppercase();
    matches!(upper.as_str(), "TRUE" | "FALSE" | "NULL")
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

/// Tokens that should not have a preceding space.
fn no_space_before(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Comma
            | TokenKind::Semicolon
            | TokenKind::ClosingRoundBracket
            | TokenKind::ClosingSquareBracket
            | TokenKind::ClosingCurlyBrace
            | TokenKind::Dot
            | TokenKind::DoubleColon
    )
}


/// Tokens after which no space should be emitted.
fn no_space_after(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::OpeningRoundBracket
            | TokenKind::OpeningSquareBracket
            | TokenKind::OpeningCurlyBrace
            | TokenKind::Dot
            | TokenKind::DoubleColon
            | TokenKind::At
            | TokenKind::DoubleAt
    )
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

pub fn format_node(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    match tree.kind {
        SyntaxKind::File => format_file(tree, ctx),
        SyntaxKind::QueryList => format_query_list(tree, ctx),
        SyntaxKind::SelectStatement => format_select_statement(tree, ctx),
        SyntaxKind::SelectClause => format_select_clause(tree, ctx),
        SyntaxKind::FromClause => format_simple_clause(tree, ctx),
        SyntaxKind::PrewhereClause => format_simple_clause(tree, ctx),
        SyntaxKind::WhereClause => format_simple_clause(tree, ctx),
        SyntaxKind::HavingClause => format_simple_clause(tree, ctx),
        SyntaxKind::GroupByClause => format_group_by_clause(tree, ctx),
        SyntaxKind::OrderByClause => format_order_by_clause(tree, ctx),
        SyntaxKind::LimitClause => format_simple_clause(tree, ctx),
        SyntaxKind::LimitByClause => format_limit_by_clause(tree, ctx),
        SyntaxKind::SettingsClause => format_settings_clause(tree, ctx),
        SyntaxKind::WithClause => format_with_clause(tree, ctx),
        SyntaxKind::JoinClause => format_join_clause(tree, ctx),
        SyntaxKind::ArrayJoinClause => format_simple_clause(tree, ctx),
        SyntaxKind::JoinType => format_inline(tree, ctx),
        SyntaxKind::JoinConstraint => format_inline(tree, ctx),
        SyntaxKind::ColumnList => format_comma_list(tree, ctx),
        SyntaxKind::ExpressionList => format_inline_comma_list(tree, ctx),
        SyntaxKind::OrderByList => format_comma_list(tree, ctx),
        SyntaxKind::GroupByList => format_comma_list(tree, ctx),
        SyntaxKind::SettingList => format_comma_list(tree, ctx),
        SyntaxKind::BinaryExpression => format_binary_expression(tree, ctx),
        SyntaxKind::UnaryExpression => format_unary_expression(tree, ctx),
        SyntaxKind::FunctionCall | SyntaxKind::AggregateFunction => {
            format_function_call(tree, ctx)
        }
        SyntaxKind::CaseExpression => format_case_expression(tree, ctx),
        SyntaxKind::WhenClause => format_when_clause(tree, ctx),
        SyntaxKind::SubqueryExpression => format_subquery(tree, ctx),
        SyntaxKind::CastExpression => format_inline(tree, ctx),
        SyntaxKind::ColumnAlias => format_inline(tree, ctx),
        SyntaxKind::TableAlias => format_inline(tree, ctx),
        SyntaxKind::OrderByItem => format_inline(tree, ctx),
        SyntaxKind::SettingItem => format_inline(tree, ctx),
        SyntaxKind::WithExpressionItem => format_inline(tree, ctx),
        SyntaxKind::BetweenExpression => format_inline(tree, ctx),
        SyntaxKind::InExpression => format_inline(tree, ctx),
        SyntaxKind::IsNullExpression => format_inline(tree, ctx),
        SyntaxKind::LikeExpression => format_inline(tree, ctx),
        SyntaxKind::IntervalExpression => format_inline(tree, ctx),
        SyntaxKind::LambdaExpression => format_inline(tree, ctx),
        SyntaxKind::TupleExpression => format_paren_list(tree, ctx),
        SyntaxKind::ArrayExpression => format_bracket_list(tree, ctx),
        SyntaxKind::ArrayAccessExpression => format_inline_no_spaces(tree, ctx),
        SyntaxKind::MapExpression => format_brace_list(tree, ctx),
        SyntaxKind::TableIdentifier => format_inline(tree, ctx),
        SyntaxKind::TableExpression => format_inline(tree, ctx),
        SyntaxKind::TableFunction => format_function_call(tree, ctx),
        SyntaxKind::QualifiedName => format_inline_no_spaces(tree, ctx),
        SyntaxKind::ColumnReference => format_inline(tree, ctx),
        SyntaxKind::DataType => format_data_type(tree, ctx),
        SyntaxKind::DataTypeParameters => format_data_type(tree, ctx),
        SyntaxKind::UsingList => format_inline(tree, ctx),

        // INSERT
        SyntaxKind::InsertStatement => format_insert_statement(tree, ctx),
        SyntaxKind::InsertColumnsClause => format_inline_comma_list(tree, ctx),
        SyntaxKind::InsertValuesClause => format_values_clause(tree, ctx),
        SyntaxKind::ValueRow => format_paren_list(tree, ctx),
        SyntaxKind::InsertFormatClause => format_simple_clause(tree, ctx),
        SyntaxKind::FormatClause => format_simple_clause(tree, ctx),

        // CREATE / DDL
        SyntaxKind::CreateStatement => format_create_statement(tree, ctx),
        SyntaxKind::ColumnDefinitionList => format_column_def_list(tree, ctx),
        SyntaxKind::ColumnDefinition => format_inline(tree, ctx),
        SyntaxKind::ColumnDefault => format_inline(tree, ctx),
        SyntaxKind::ColumnCodec => format_inline_tight_parens(tree, ctx),
        SyntaxKind::ColumnTtl => format_inline(tree, ctx),
        SyntaxKind::ColumnComment => format_inline(tree, ctx),
        SyntaxKind::EngineClause => format_engine_clause(tree, ctx),
        SyntaxKind::OrderByDefinition => format_simple_clause(tree, ctx),
        SyntaxKind::PartitionByDefinition => format_simple_clause(tree, ctx),
        SyntaxKind::PrimaryKeyDefinition => format_simple_clause(tree, ctx),
        SyntaxKind::SampleByDefinition => format_simple_clause(tree, ctx),
        SyntaxKind::TtlDefinition => format_simple_clause(tree, ctx),
        SyntaxKind::OnClusterClause => format_inline(tree, ctx),
        SyntaxKind::IfNotExistsClause => format_inline(tree, ctx),
        SyntaxKind::IfExistsClause => format_inline(tree, ctx),
        SyntaxKind::AsClause => format_simple_clause(tree, ctx),
        SyntaxKind::TableDefinition => format_create_statement(tree, ctx),
        SyntaxKind::DatabaseDefinition => format_create_statement(tree, ctx),
        SyntaxKind::ViewDefinition => format_create_statement(tree, ctx),
        SyntaxKind::MaterializedViewDefinition => format_create_statement(tree, ctx),
        SyntaxKind::DictionaryDefinition => format_create_statement(tree, ctx),
        SyntaxKind::FunctionDefinition => format_inline(tree, ctx),
        SyntaxKind::IndexDefinition => format_inline_tight_parens(tree, ctx),
        SyntaxKind::ProjectionDefinition => format_inline(tree, ctx),
        SyntaxKind::ConstraintDefinition => format_inline(tree, ctx),

        // ALTER
        SyntaxKind::AlterStatement => format_alter_statement(tree, ctx),
        SyntaxKind::AlterCommandList => format_alter_command_list(tree, ctx),
        SyntaxKind::AlterAddColumn
        | SyntaxKind::AlterDropColumn
        | SyntaxKind::AlterModifyColumn
        | SyntaxKind::AlterRenameColumn
        | SyntaxKind::AlterClearColumn
        | SyntaxKind::AlterCommentColumn
        | SyntaxKind::AlterAddIndex
        | SyntaxKind::AlterDropIndex
        | SyntaxKind::AlterClearIndex
        | SyntaxKind::AlterMaterializeIndex
        | SyntaxKind::AlterAddProjection
        | SyntaxKind::AlterDropProjection
        | SyntaxKind::AlterAddConstraint
        | SyntaxKind::AlterDropConstraint
        | SyntaxKind::AlterModifyOrderBy
        | SyntaxKind::AlterModifyTtl
        | SyntaxKind::AlterModifySetting
        | SyntaxKind::AlterResetSetting
        | SyntaxKind::AlterDropPartition
        | SyntaxKind::AlterAttachPartition
        | SyntaxKind::AlterDetachPartition
        | SyntaxKind::AlterFreezePartition
        | SyntaxKind::AlterDeleteWhere
        | SyntaxKind::AlterUpdateWhere => format_inline(tree, ctx),

        // UNION
        SyntaxKind::UnionClause => format_union_clause(tree, ctx),

        // Simple DDL
        SyntaxKind::UseStatement
        | SyntaxKind::DropStatement
        | SyntaxKind::TruncateStatement
        | SyntaxKind::ExistsStatement
        | SyntaxKind::CheckStatement
        | SyntaxKind::RenameStatement
        | SyntaxKind::OptimizeStatement
        | SyntaxKind::DeleteStatement => format_simple_clause(tree, ctx),
        SyntaxKind::SetStatement => format_set_statement(tree, ctx),
        SyntaxKind::RenameItem => format_inline(tree, ctx),
        SyntaxKind::IdentifierList => format_inline_comma_list(tree, ctx),
        SyntaxKind::PartitionExpression => format_inline(tree, ctx),
        SyntaxKind::Assignment => format_inline(tree, ctx),
        SyntaxKind::AssignmentList => format_inline_comma_list(tree, ctx),
        SyntaxKind::SetClause => format_inline(tree, ctx),

        // EXPLAIN / DESCRIBE / SHOW
        SyntaxKind::ExplainStatement => format_explain_statement(tree, ctx),
        SyntaxKind::DescribeStatement => format_simple_clause(tree, ctx),
        SyntaxKind::ShowStatement => format_simple_clause(tree, ctx),
        SyntaxKind::ExplainKind => format_inline(tree, ctx),
        SyntaxKind::ShowTarget => format_inline(tree, ctx),
        SyntaxKind::LikeClause => format_inline(tree, ctx),
        SyntaxKind::FromDatabaseClause => format_inline(tree, ctx),

        _ => format_passthrough(tree, ctx),
    }
}

// ---------------------------------------------------------------------------
// Top-level
// ---------------------------------------------------------------------------

fn format_file(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut had_statement = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_token(&t.text);
                ctx.write_newline();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Semicolon => {
                ctx.write_token(";");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                if had_statement {
                    ctx.write_newline();
                    ctx.write_newline();
                }
                had_statement = true;
                format_node(subtree, ctx);
            }
        }
    }
}

fn format_query_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut first = true;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Semicolon => {
                ctx.write_token(";");
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                if !first {
                    ctx.write_newline();
                    ctx.write_newline();
                }
                first = false;
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SELECT statement -- clause per line
// ---------------------------------------------------------------------------

fn format_select_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut first_clause = true;

    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Semicolon => {
                // Semicolons stay on the same line as the last clause
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                if !first_clause {
                    ctx.write_newline();
                }
                first_clause = false;
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SELECT clause
// ---------------------------------------------------------------------------

fn format_select_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    // Emit keywords (SELECT, DISTINCT) then indent the column list
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                // Keywords like SELECT, DISTINCT
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree)
                if subtree.kind == SyntaxKind::ColumnList =>
            {
                ctx.write_newline();
                ctx.indent();
                format_node(subtree, ctx);
                ctx.dedent();
            }
            SyntaxChild::Tree(subtree) => {
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Simple clause: keyword(s) + expression on the same line
// e.g. FROM table, WHERE cond, LIMIT 10
// ---------------------------------------------------------------------------

fn format_simple_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => {
                if need_sep && !no_space_before(t) {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                need_sep = !no_space_after(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if need_sep {
                    ctx.write_space();
                }
                format_node(subtree, ctx);
                need_sep = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GROUP BY clause -- keyword + list on next line if multiple items
// ---------------------------------------------------------------------------

fn format_group_by_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    // Check if items are directly in the clause (no GroupByList wrapper)
    let has_list = tree.children.iter().any(|c| {
        matches!(c, SyntaxChild::Tree(t) if t.kind == SyntaxKind::GroupByList)
    });
    let direct_item_count = if !has_list {
        tree.children
            .iter()
            .filter(|c| matches!(c, SyntaxChild::Tree(_)))
            .count()
    } else {
        0
    };
    let multi_item = direct_item_count > 1;
    let mut after_keywords = false;

    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                if multi_item {
                    ctx.write_newline();
                } else {
                    ctx.write_space();
                }
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree)
                if subtree.kind == SyntaxKind::GroupByList =>
            {
                let item_count = count_list_items(subtree);
                if item_count > 1 {
                    ctx.write_newline();
                    ctx.indent();
                    format_node(subtree, ctx);
                    ctx.dedent();
                } else {
                    format_node(subtree, ctx);
                }
            }
            SyntaxChild::Tree(subtree) => {
                if !after_keywords && multi_item {
                    ctx.write_newline();
                    ctx.indent();
                    after_keywords = true;
                }
                format_node(subtree, ctx);
            }
        }
    }
    if after_keywords && multi_item {
        ctx.dedent();
    }
}

// ---------------------------------------------------------------------------
// ORDER BY clause
// ---------------------------------------------------------------------------

fn format_order_by_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let has_list = tree.children.iter().any(|c| {
        matches!(c, SyntaxChild::Tree(t) if t.kind == SyntaxKind::OrderByList)
    });
    let direct_item_count = if !has_list {
        tree.children
            .iter()
            .filter(|c| matches!(c, SyntaxChild::Tree(_)))
            .count()
    } else {
        0
    };
    let multi_item = direct_item_count > 1;
    let mut after_keywords = false;

    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                if multi_item {
                    ctx.write_newline();
                } else {
                    ctx.write_space();
                }
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree)
                if subtree.kind == SyntaxKind::OrderByList =>
            {
                let item_count = count_list_items(subtree);
                if item_count > 1 {
                    ctx.write_newline();
                    ctx.indent();
                    format_node(subtree, ctx);
                    ctx.dedent();
                } else {
                    format_node(subtree, ctx);
                }
            }
            SyntaxChild::Tree(subtree) => {
                if !after_keywords && multi_item {
                    ctx.write_newline();
                    ctx.indent();
                    after_keywords = true;
                }
                format_node(subtree, ctx);
            }
        }
    }
    if after_keywords && multi_item {
        ctx.dedent();
    }
}

// ---------------------------------------------------------------------------
// LIMIT BY clause
// ---------------------------------------------------------------------------

fn format_limit_by_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                if need_sep {
                    ctx.write_space();
                }
                ctx.write_keyword(&t.text);
                need_sep = true;
            }
            SyntaxChild::Token(t) => {
                if need_sep && !no_space_before(t) {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                need_sep = !no_space_after(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if need_sep {
                    ctx.write_space();
                }
                format_node(subtree, ctx);
                need_sep = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SETTINGS clause
// ---------------------------------------------------------------------------

fn format_settings_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree)
                if subtree.kind == SyntaxKind::SettingList =>
            {
                ctx.write_newline();
                ctx.indent();
                format_node(subtree, ctx);
                ctx.dedent();
            }
            SyntaxChild::Tree(subtree) => {
                ctx.write_space();
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WITH clause
// ---------------------------------------------------------------------------

fn format_with_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                ctx.write_newline();
                ctx.indent();
                format_node(subtree, ctx);
                ctx.dedent();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JOIN clause
// ---------------------------------------------------------------------------

fn format_join_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                if need_sep {
                    ctx.write_space();
                }
                ctx.write_keyword(&t.text);
                need_sep = true;
            }
            SyntaxChild::Token(t) => {
                if need_sep && !no_space_before(t) {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                need_sep = !no_space_after(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if need_sep {
                    ctx.write_space();
                }
                format_node(subtree, ctx);
                need_sep = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Comma-separated list -- one item per line
// ---------------------------------------------------------------------------

fn format_comma_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut after_comma = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_newline();
                after_comma = true;
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                // ColumnAlias and similar nodes follow their expression
                // without a comma -- they need a space, not a newline
                if !after_comma {
                    ctx.write_space();
                }
                after_comma = false;
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Inline comma list -- items on one line separated by ", "
// Used for ExpressionList (function args, IN lists, etc.)
// ---------------------------------------------------------------------------

fn format_inline_comma_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

fn format_binary_expression(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    // Binary expression children: [lhs, operator, rhs]
    // We want spaces around the operator
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                // Keyword operators: AND, OR
                ctx.write_space();
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) if is_operator(t) => {
                ctx.write_space();
                ctx.write_token(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_unary_expression(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                // NOT keyword
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Minus => {
                // Unary minus: no space between - and operand
                ctx.write_token("-");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_function_call(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::OpeningRoundBracket =>
            {
                ctx.write_token("(");
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::ClosingRoundBracket =>
            {
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                // Function name -- don't uppercase, it's an identifier
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_case_expression(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                let upper = t.text.to_uppercase();
                match upper.as_str() {
                    "CASE" => {
                        ctx.write_keyword(&t.text);
                        ctx.indent();
                    }
                    "ELSE" => {
                        ctx.write_newline();
                        ctx.write_keyword(&t.text);
                        ctx.write_space();
                    }
                    "END" => {
                        ctx.dedent();
                        ctx.write_newline();
                        ctx.write_keyword(&t.text);
                    }
                    _ => {
                        ctx.write_space();
                        ctx.write_keyword(&t.text);
                    }
                }
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree)
                if subtree.kind == SyntaxKind::WhenClause =>
            {
                ctx.write_newline();
                format_node(subtree, ctx);
            }
            SyntaxChild::Tree(subtree) => {
                format_node(subtree, ctx);
            }
        }
    }
}

fn format_when_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                let upper = t.text.to_uppercase();
                match upper.as_str() {
                    "WHEN" => {
                        ctx.write_keyword(&t.text);
                        ctx.write_space();
                    }
                    "THEN" => {
                        ctx.write_space();
                        ctx.write_keyword(&t.text);
                        ctx.write_space();
                    }
                    _ => {
                        ctx.write_keyword(&t.text);
                        ctx.write_space();
                    }
                }
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_subquery(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::OpeningRoundBracket =>
            {
                ctx.write_token("(");
                ctx.write_newline();
                ctx.indent();
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::ClosingRoundBracket =>
            {
                ctx.dedent();
                ctx.write_newline();
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Parenthesized / bracketed inline lists
// ---------------------------------------------------------------------------

fn format_paren_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::OpeningRoundBracket =>
            {
                ctx.write_token("(");
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::ClosingRoundBracket =>
            {
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_bracket_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::OpeningSquareBracket =>
            {
                ctx.write_token("[");
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::ClosingSquareBracket =>
            {
                ctx.write_token("]");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_brace_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Colon => {
                ctx.write_token(":");
                ctx.write_space();
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::OpeningCurlyBrace =>
            {
                ctx.write_token("{");
            }
            SyntaxChild::Token(t)
                if t.kind == TokenKind::ClosingCurlyBrace =>
            {
                ctx.write_token("}");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Data type formatting -- preserves original casing, tight parentheses
// ---------------------------------------------------------------------------

fn format_data_type(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::OpeningRoundBracket => {
                ctx.write_token("(");
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::ClosingRoundBracket => {
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) => {
                // Type names are identifiers — preserve original casing
                ctx.write_token(&t.text);
            }
            SyntaxChild::Tree(subtree) => {
                // No space before nested type parameters
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Inline formatting with tight parens -- no space before ( after identifiers
// Used for CODEC(...), INDEX ... TYPE bloom_filter(...), etc.
// ---------------------------------------------------------------------------

fn format_inline_tight_parens(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut prev_kind: Option<TokenKind> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => {
                let suppress_space = t.kind == TokenKind::OpeningRoundBracket
                    && matches!(prev_kind, Some(TokenKind::BareWord | TokenKind::QuotedIdentifier));
                if !no_space_before(t) && !suppress_space {
                    if let Some(pk) = prev_kind {
                        if !no_space_after(pk) {
                            ctx.write_space();
                        }
                    }
                }
                emit_token(t, ctx);
                prev_kind = Some(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if let Some(pk) = prev_kind {
                    if !no_space_after(pk) {
                        ctx.write_space();
                    }
                }
                format_node(subtree, ctx);
                prev_kind = Some(TokenKind::BareWord);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Engine clause -- preserves engine name casing, tight parens for args
// ---------------------------------------------------------------------------

fn format_engine_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    let mut prev_kind: Option<TokenKind> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => {
                let suppress_space = t.kind == TokenKind::OpeningRoundBracket
                    && matches!(prev_kind, Some(TokenKind::BareWord | TokenKind::QuotedIdentifier));
                if need_sep && !no_space_before(t) && !suppress_space {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                prev_kind = Some(t.kind);
                need_sep = !no_space_after(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if need_sep {
                    ctx.write_space();
                }
                format_node(subtree, ctx);
                prev_kind = None;
                need_sep = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Inline formatting -- tokens with single spaces between them
// ---------------------------------------------------------------------------

fn format_inline(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut prev_kind: Option<TokenKind> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => {
                if !no_space_before(t) {
                    if let Some(pk) = prev_kind {
                        if !no_space_after(pk) {
                            ctx.write_space();
                        }
                    }
                }
                emit_token(t, ctx);
                prev_kind = Some(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if let Some(pk) = prev_kind {
                    if !no_space_after(pk) {
                        ctx.write_space();
                    }
                }
                format_node(subtree, ctx);
                // After a subtree, we don't know the last token kind
                // so default to needing a space
                prev_kind = Some(TokenKind::BareWord);
            }
        }
    }
}

/// Like format_inline but with no spaces at all (for a.b.c, arr[i])
fn format_inline_no_spaces(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Passthrough -- resilience backstop
// ---------------------------------------------------------------------------

fn format_passthrough(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn emit_token(token: &Token, ctx: &mut FormatterContext) {
    if token.kind == TokenKind::Whitespace {
        return;
    }
    if token.kind == TokenKind::BareWord && is_keyword(&token.text) && !is_value_keyword(&token.text) {
        ctx.write_keyword(&token.text);
    } else {
        ctx.write_token(&token.text);
    }
}

fn is_operator(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Asterisk
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Equals
            | TokenKind::NotEquals
            | TokenKind::Less
            | TokenKind::Greater
            | TokenKind::LessOrEquals
            | TokenKind::GreaterOrEquals
            | TokenKind::Spaceship
            | TokenKind::Concatenation
    )
}

/// Count the number of non-comma, non-whitespace tree children in a list node.
fn count_list_items(tree: &SyntaxTree) -> usize {
    tree.children
        .iter()
        .filter(|c| matches!(c, SyntaxChild::Tree(_)))
        .count()
}

// ---------------------------------------------------------------------------
// INSERT statement
// ---------------------------------------------------------------------------

fn format_insert_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut first_clause = true;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                if !first_clause {
                    ctx.write_space();
                }
                ctx.write_keyword(&t.text);
                first_clause = false;
            }
            SyntaxChild::Token(t) => {
                if !first_clause && !no_space_before(t) {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                first_clause = false;
            }
            SyntaxChild::Tree(subtree) => {
                match subtree.kind {
                    SyntaxKind::InsertValuesClause => {
                        ctx.write_newline();
                        format_node(subtree, ctx);
                    }
                    SyntaxKind::SelectStatement => {
                        ctx.write_newline();
                        format_node(subtree, ctx);
                    }
                    SyntaxKind::SettingsClause => {
                        ctx.write_newline();
                        format_node(subtree, ctx);
                    }
                    _ => {
                        ctx.write_space();
                        format_node(subtree, ctx);
                    }
                }
                first_clause = false;
            }
        }
    }
}

fn format_values_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                need_sep = true;
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                need_sep = true;
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                if need_sep {
                    ctx.write_space();
                }
                format_node(subtree, ctx);
                need_sep = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CREATE statement -- clause per line
// ---------------------------------------------------------------------------

fn format_create_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut first_clause = true;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comment => {
                ctx.write_space();
                ctx.write_token(&t.text);
            }
            SyntaxChild::Token(t) => {
                if !first_clause && !no_space_before(t) {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                first_clause = false;
            }
            SyntaxChild::Tree(subtree) => {
                match subtree.kind {
                    SyntaxKind::ColumnDefinitionList
                    | SyntaxKind::EngineClause
                    | SyntaxKind::OrderByDefinition
                    | SyntaxKind::PartitionByDefinition
                    | SyntaxKind::PrimaryKeyDefinition
                    | SyntaxKind::SampleByDefinition
                    | SyntaxKind::TtlDefinition
                    | SyntaxKind::SettingsClause
                    | SyntaxKind::AsClause => {
                        ctx.write_newline();
                        format_node(subtree, ctx);
                    }
                    _ => {
                        if !first_clause {
                            ctx.write_space();
                        }
                        format_node(subtree, ctx);
                    }
                }
                first_clause = false;
            }
        }
    }
}

fn format_column_def_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::OpeningRoundBracket => {
                ctx.write_token("(");
                ctx.indent();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::ClosingRoundBracket => {
                ctx.dedent();
                ctx.write_newline();
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                ctx.write_newline();
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ALTER statement
// ---------------------------------------------------------------------------

fn format_alter_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                if need_sep {
                    ctx.write_space();
                }
                ctx.write_keyword(&t.text);
                need_sep = true;
            }
            SyntaxChild::Token(t) => {
                if need_sep && !no_space_before(t) {
                    ctx.write_space();
                }
                emit_token(t, ctx);
                need_sep = !no_space_after(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                match subtree.kind {
                    SyntaxKind::AlterCommandList => {
                        ctx.write_newline();
                        ctx.indent();
                        format_node(subtree, ctx);
                        ctx.dedent();
                    }
                    _ => {
                        if need_sep {
                            ctx.write_space();
                        }
                        format_node(subtree, ctx);
                        need_sep = true;
                    }
                }
            }
        }
    }
}

fn format_alter_command_list(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut after_comma = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_newline();
                after_comma = true;
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                after_comma = false;
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SET statement
// ---------------------------------------------------------------------------

fn format_set_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                ctx.write_newline();
                ctx.indent();
            }
            SyntaxChild::Token(t) if t.kind == TokenKind::Comma => {
                ctx.write_token(",");
                ctx.write_newline();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                format_node(subtree, ctx);
            }
        }
    }
    ctx.dedent();
}

// ---------------------------------------------------------------------------
// UNION clause
// ---------------------------------------------------------------------------

fn format_union_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut after_first_select = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                if after_first_select {
                    // This is the UNION/EXCEPT/INTERSECT keyword or ALL/DISTINCT
                    ctx.write_space();
                    ctx.write_keyword(&t.text);
                } else {
                    ctx.write_keyword(&t.text);
                    ctx.write_space();
                }
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) if subtree.kind == SyntaxKind::SelectStatement
                || subtree.kind == SyntaxKind::UnionClause => {
                if after_first_select {
                    ctx.write_newline();
                }
                format_node(subtree, ctx);
                after_first_select = true;
            }
            SyntaxChild::Tree(subtree) => {
                format_node(subtree, ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// EXPLAIN statement
// ---------------------------------------------------------------------------

fn format_explain_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut had_kind = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == TokenKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == TokenKind::BareWord => {
                ctx.write_keyword(&t.text);
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) if subtree.kind == SyntaxKind::ExplainKind => {
                format_node(subtree, ctx);
                had_kind = true;
            }
            SyntaxChild::Tree(subtree) if subtree.kind == SyntaxKind::SelectStatement
                || subtree.kind == SyntaxKind::ShowStatement
                || subtree.kind == SyntaxKind::DescribeStatement => {
                ctx.write_newline();
                ctx.indent();
                format_node(subtree, ctx);
                ctx.dedent();
            }
            SyntaxChild::Tree(subtree) => {
                ctx.write_space();
                format_node(subtree, ctx);
            }
        }
    }
}
