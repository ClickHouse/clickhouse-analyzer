use super::context::FormatterContext;
use crate::lexer::token::Token;
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
    "PRECEDING", "FOLLOWING", "CURRENT", "DIV", "MOD",
    "SYNTAX", "TREE", "OVERRIDE", "ENGINES", "FOR", "PART",
    "MATERIALIZE", "SETTING", "RESET",
    "FILL", "STEP", "INTERPOLATE", "OPTION",
    "IDENTIFIED", "HOST", "KEYED",
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
        SyntaxKind::Comma
            | SyntaxKind::Semicolon
            | SyntaxKind::ClosingRoundBracket
            | SyntaxKind::ClosingSquareBracket
            | SyntaxKind::ClosingCurlyBrace
            | SyntaxKind::Dot
            | SyntaxKind::DoubleColon
    )
}


/// Tokens after which no space should be emitted.
fn no_space_after(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OpeningRoundBracket
            | SyntaxKind::OpeningSquareBracket
            | SyntaxKind::OpeningCurlyBrace
            | SyntaxKind::Dot
            | SyntaxKind::DoubleColon
            | SyntaxKind::At
            | SyntaxKind::DoubleAt
    )
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

/// Returns true if any direct child is an Error node.
fn has_error_child(tree: &SyntaxTree) -> bool {
    tree.children.iter().any(|c| matches!(c, SyntaxChild::Tree(t) if t.kind == SyntaxKind::Error))
}

pub fn format_node(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    // If a non-root node contains Error children, emit it verbatim.
    // Reformatting error-recovery structures changes token boundaries on
    // re-parse, breaking idempotency.
    if tree.kind != SyntaxKind::File
        && tree.kind != SyntaxKind::QueryList
        && has_error_child(tree)
    {
        format_error_verbatim(tree, ctx);
        return;
    }

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
        SyntaxKind::WindowClause => format_simple_clause(tree, ctx),
        SyntaxKind::WindowDefinition => format_inline(tree, ctx),
        SyntaxKind::WindowFrame => format_inline(tree, ctx),
        SyntaxKind::WindowSpec => format_inline(tree, ctx),
        SyntaxKind::WindowExpression => format_inline(tree, ctx),
        SyntaxKind::SampleClause => format_inline(tree, ctx),
        SyntaxKind::WithFillClause => format_inline(tree, ctx),
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
        SyntaxKind::ColumnAlias => format_alias(tree, ctx),
        SyntaxKind::TableAlias => format_alias(tree, ctx),
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
        SyntaxKind::QueryParameterExpression => format_inline_no_spaces(tree, ctx),
        SyntaxKind::TableIdentifier => format_identifier_inline(tree, ctx),
        SyntaxKind::TableExpression => format_inline(tree, ctx),
        SyntaxKind::TableFunction => format_function_call(tree, ctx),
        SyntaxKind::QualifiedName => format_identifier_inline_no_spaces(tree, ctx),
        SyntaxKind::ColumnReference => format_identifier_inline(tree, ctx),
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
        SyntaxKind::DictionarySource => format_inline_tight_parens(tree, ctx),
        SyntaxKind::DictionarySourceType => format_inline_tight_parens(tree, ctx),
        SyntaxKind::DictionaryLayout => format_inline_tight_parens(tree, ctx),
        SyntaxKind::DictionaryLayoutType => format_inline_tight_parens(tree, ctx),
        SyntaxKind::DictionaryLifetime => format_inline_tight_parens(tree, ctx),
        SyntaxKind::DictionaryRange => format_inline_tight_parens(tree, ctx),
        SyntaxKind::DictionaryKeyValue => format_inline(tree, ctx),
        SyntaxKind::FunctionDefinition => format_inline(tree, ctx),
        SyntaxKind::IndexDefinition => format_inline_tight_parens(tree, ctx),
        SyntaxKind::ProjectionDefinition => format_inline(tree, ctx),
        SyntaxKind::ConstraintDefinition => format_inline(tree, ctx),
        SyntaxKind::CreateIndexStatement => format_simple_clause(tree, ctx),

        // Access control
        SyntaxKind::CreateUserStatement
        | SyntaxKind::CreateRoleStatement
        | SyntaxKind::CreateQuotaStatement
        | SyntaxKind::CreateRowPolicyStatement
        | SyntaxKind::CreateSettingsProfileStatement
        | SyntaxKind::AlterUserStatement
        | SyntaxKind::DropAccessEntityStatement => format_inline(tree, ctx),

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
        | SyntaxKind::AlterModifyComment
        | SyntaxKind::AlterModifyQuery
        | SyntaxKind::AlterMaterializeProjection
        | SyntaxKind::AlterMaterializeTtl
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
        | SyntaxKind::DeleteStatement
        | SyntaxKind::AttachStatement
        | SyntaxKind::DetachStatement
        | SyntaxKind::ExchangeStatement
        | SyntaxKind::UndropStatement
        | SyntaxKind::BackupStatement
        | SyntaxKind::RestoreStatement => format_simple_clause(tree, ctx),
        | SyntaxKind::GrantStatement
        | SyntaxKind::RevokeStatement => format_simple_clause(tree, ctx),
        | SyntaxKind::SystemStatement
        | SyntaxKind::KillStatement => format_simple_clause(tree, ctx),
        SyntaxKind::SystemCommand => format_inline(tree, ctx),
        SyntaxKind::KillTarget => format_inline(tree, ctx),
        SyntaxKind::SetStatement => format_set_statement(tree, ctx),
        SyntaxKind::RenameItem => format_inline(tree, ctx),
        SyntaxKind::PrivilegeList => format_inline_comma_list(tree, ctx),
        SyntaxKind::Privilege => format_inline(tree, ctx),
        SyntaxKind::GrantTarget => format_inline(tree, ctx),
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

        // Error nodes: emit original source text verbatim.
        // Reformatting error regions changes token boundaries on re-parse,
        // breaking formatter idempotency.
        SyntaxKind::Error => format_error_verbatim(tree, ctx),

        _ => format_passthrough(tree, ctx),
    }
}

// ---------------------------------------------------------------------------
// Top-level
// ---------------------------------------------------------------------------

fn format_file(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    // If any direct children are Error nodes, emit the entire file verbatim.
    // Error tokens at file level intermixed with whitespace can change meaning
    // when whitespace is dropped (e.g., '-' + whitespace + '- ' → '--' comment).
    if has_error_child(tree) {
        format_error_verbatim(tree, ctx);
        return;
    }

    let mut had_statement = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                if ctx.take_pending_blank_line() && !ctx.is_at_line_start() {
                    ctx.write_newline();
                }
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Semicolon => {
                ctx.write_token(";");
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
                if had_statement {
                    ctx.write_newline();
                    ctx.write_newline();
                } else if ctx.take_pending_blank_line() {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Semicolon => {
                ctx.write_token(";");
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Semicolon => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                // Keywords like SELECT, DISTINCT
                ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                if need_sep {
                    ctx.write_space();
                }
                ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                if need_sep {
                    ctx.write_space();
                }
                ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                // Keyword operators: AND, OR
                ctx.write_space();
                ctx.write_keyword(t.text(ctx.source));
                ctx.write_space();
            }
            SyntaxChild::Token(t) if is_operator(t) => {
                ctx.write_space();
                ctx.write_token(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                // NOT keyword
                ctx.write_keyword(t.text(ctx.source));
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Minus => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::OpeningRoundBracket =>
            {
                ctx.write_token("(");
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::ClosingRoundBracket =>
            {
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                // Function name -- don't uppercase, it's an identifier
                ctx.write_token(t.text(ctx.source));
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

fn format_case_expression(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                let upper = t.text(ctx.source).to_uppercase();
                match upper.as_str() {
                    "CASE" => {
                        ctx.write_keyword(t.text(ctx.source));
                        ctx.indent();
                    }
                    "ELSE" => {
                        ctx.write_newline();
                        ctx.write_keyword(t.text(ctx.source));
                        ctx.write_space();
                    }
                    "END" => {
                        ctx.dedent();
                        ctx.write_newline();
                        ctx.write_keyword(t.text(ctx.source));
                    }
                    _ => {
                        ctx.write_space();
                        ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                let upper = t.text(ctx.source).to_uppercase();
                match upper.as_str() {
                    "WHEN" => {
                        ctx.write_keyword(t.text(ctx.source));
                        ctx.write_space();
                    }
                    "THEN" => {
                        ctx.write_space();
                        ctx.write_keyword(t.text(ctx.source));
                        ctx.write_space();
                    }
                    _ => {
                        ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::OpeningRoundBracket =>
            {
                ctx.write_token("(");
                ctx.write_newline();
                ctx.indent();
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::ClosingRoundBracket =>
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::OpeningRoundBracket =>
            {
                ctx.write_token("(");
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::ClosingRoundBracket =>
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::OpeningSquareBracket =>
            {
                ctx.write_token("[");
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::ClosingSquareBracket =>
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Colon => {
                ctx.write_token(":");
                ctx.write_space();
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::OpeningCurlyBrace =>
            {
                ctx.write_token("{");
            }
            SyntaxChild::Token(t)
                if t.kind == SyntaxKind::ClosingCurlyBrace =>
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
                ctx.write_token(",");
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::OpeningRoundBracket => {
                ctx.write_token("(");
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::ClosingRoundBracket => {
                ctx.write_token(")");
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) => {
                // Type names are identifiers — preserve original casing
                ctx.write_token(t.text(ctx.source));
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
    let mut prev_kind: Option<SyntaxKind> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) => {
                let suppress_space = t.kind == SyntaxKind::OpeningRoundBracket
                    && matches!(prev_kind, Some(SyntaxKind::BareWord | SyntaxKind::QuotedIdentifier));
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
                prev_kind = Some(SyntaxKind::BareWord);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Engine clause -- preserves engine name casing, tight parens for args
// ---------------------------------------------------------------------------

fn format_engine_clause(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    let mut prev_kind: Option<SyntaxKind> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) => {
                let suppress_space = t.kind == SyntaxKind::OpeningRoundBracket
                    && matches!(prev_kind, Some(SyntaxKind::BareWord | SyntaxKind::QuotedIdentifier));
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
    let mut prev_kind: Option<SyntaxKind> = None;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
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
                prev_kind = Some(SyntaxKind::BareWord);
            }
        }
    }
}

/// Like format_inline but with no spaces at all (for a.b.c, arr[i])
fn format_inline_no_spaces(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

/// Wrap format_inline with identifier-context so BareWords matching keywords
/// (e.g. a column literally named `type`) are not re-cased.
fn format_identifier_inline(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    ctx.enter_identifier();
    format_inline(tree, ctx);
    ctx.exit_identifier();
}

/// Wrap format_inline_no_spaces with identifier-context for qualified names.
fn format_identifier_inline_no_spaces(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    ctx.enter_identifier();
    format_inline_no_spaces(tree, ctx);
    ctx.exit_identifier();
}

/// Format a TableAlias or ColumnAlias: the optional `AS` is a keyword, the
/// alias name itself is an identifier and must keep its source case.
fn format_alias(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut prev_kind: Option<SyntaxKind> = None;
    let mut seen_as = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) => {
                if !no_space_before(t) {
                    if let Some(pk) = prev_kind {
                        if !no_space_after(pk) {
                            ctx.write_space();
                        }
                    }
                }
                let is_as_kw = t.kind == SyntaxKind::BareWord
                    && !seen_as
                    && t.text(ctx.source).eq_ignore_ascii_case("AS");
                if is_as_kw {
                    emit_token(t, ctx);
                    seen_as = true;
                } else if t.kind == SyntaxKind::BareWord {
                    // Alias name — always preserve case.
                    ctx.enter_identifier();
                    emit_token(t, ctx);
                    ctx.exit_identifier();
                } else {
                    emit_token(t, ctx);
                }
                prev_kind = Some(t.kind);
            }
            SyntaxChild::Tree(subtree) => {
                if let Some(pk) = prev_kind {
                    if !no_space_after(pk) {
                        ctx.write_space();
                    }
                }
                format_node(subtree, ctx);
                prev_kind = Some(SyntaxKind::BareWord);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Passthrough -- resilience backstop
// ---------------------------------------------------------------------------

/// Emit the original source text for an error node without any formatting.
/// This preserves exact token boundaries so re-parsing produces the same tree.
fn format_error_verbatim(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    if tree.start <= tree.end {
        let text = &ctx.source[tree.start as usize..tree.end as usize];
        ctx.write_raw(text);
    }
}

fn format_passthrough(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
                ctx.write_space();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => format_node(subtree, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Emit a comment, preserving its original line placement.
/// If the comment was preceded by a newline in the source, emit it on a new line.
fn emit_comment(token: &Token, ctx: &mut FormatterContext) {
    if ctx.take_pending_newline() {
        if !ctx.is_at_line_start() {
            ctx.write_newline();
        }
    } else {
        ctx.write_space();
    }
    ctx.write_token(token.text(ctx.source));
    // Line comments (--) consume the rest of the line, so we must newline after.
    if token.text(ctx.source).starts_with("--") {
        ctx.write_newline();
    }
}

fn emit_token(token: &Token, ctx: &mut FormatterContext) {
    if token.kind == SyntaxKind::Whitespace {
        return;
    }
    // Preserve case for identifiers — a BareWord inside an identifier-bearing
    // node (e.g. a table named `type`) must not be rewritten as a keyword.
    if token.kind == SyntaxKind::BareWord
        && !ctx.in_identifier()
        && is_keyword(token.text(ctx.source))
        && !is_value_keyword(token.text(ctx.source))
    {
        ctx.write_keyword(token.text(ctx.source));
    } else {
        ctx.write_token(token.text(ctx.source));
    }
}

fn is_operator(token: &Token) -> bool {
    matches!(
        token.kind,
        SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Percent
            | SyntaxKind::Equals
            | SyntaxKind::NotEquals
            | SyntaxKind::Less
            | SyntaxKind::Greater
            | SyntaxKind::LessOrEquals
            | SyntaxKind::GreaterOrEquals
            | SyntaxKind::Spaceship
            | SyntaxKind::Concatenation
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                if !first_clause {
                    ctx.write_space();
                }
                ctx.write_keyword(t.text(ctx.source));
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
                need_sep = true;
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                emit_comment(t, ctx);
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
    // Two-pass approach to align trailing comments.
    //
    // Pass 1: format each column-definition line into a string, split off any
    //         trailing comment, and record both pieces.
    // Pass 2: pad the non-comment part so all comments start at the same column.

    struct Line {
        content: String,  // formatted column def (+ comma)
        comment: Option<String>,
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut has_open = false;
    let mut has_close = false;

    // Group children into lines. Each Tree child (ColumnDefinition) starts a new line.
    // Tokens like comma attach to the previous line, comments attach as trailing.
    let mut current_parts: Vec<&SyntaxChild> = Vec::new();

    let flush_line = |parts: &mut Vec<&SyntaxChild>, lines: &mut Vec<Line>, ctx: &FormatterContext| {
        if parts.is_empty() {
            return;
        }
        let mut tmp = ctx.child();
        for child in parts.iter() {
            match child {
                SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {}
                SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {}
                SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
                    tmp.write_token(",");
                }
                SyntaxChild::Token(t) => emit_token(t, &mut tmp),
                SyntaxChild::Tree(subtree) => format_node(subtree, &mut tmp),
            }
        }

        // Extract trailing comment: could be a direct Comment token in parts,
        // or the formatted output may end with one from a nested node.
        let formatted = tmp.output().to_string();

        // Check for direct comment token in parts
        let mut comment = None;
        for child in parts.iter().rev() {
            match child {
                SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => continue,
                SyntaxChild::Token(t) if t.kind == SyntaxKind::Comment => {
                    comment = Some(t.text(ctx.source).to_string());
                    break;
                }
                _ => break,
            }
        }

        // If no direct comment, check if the formatted output ends with a line comment
        // (from a nested node that emit_comment formatted)
        if comment.is_none() {
            let trimmed = formatted.trim_end();
            // Find if a line comment got appended by emit_comment (which adds \n after --)
            // The output might be "col_b Int64 -- example2\n"
            if let Some(pos) = trimmed.rfind("-- ").or_else(|| trimmed.rfind("--")) {
                // Only if it looks like a trailing comment (not part of a string/identifier)
                let before = &trimmed[..pos];
                // Heuristic: there should be a space or line-start before --
                if before.is_empty() || before.ends_with(' ') {
                    comment = Some(trimmed[pos..].to_string());
                }
            }
        }

        let content = if let Some(ref c) = comment {
            // Strip the comment from the formatted content
            let trimmed = formatted.trim_end();
            let content = trimmed.strip_suffix(c.as_str())
                .unwrap_or(trimmed)
                .trim_end()
                .to_string();
            content
        } else {
            formatted.trim_end().to_string()
        };

        lines.push(Line { content, comment });
        parts.clear();
    };

    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {}
            SyntaxChild::Token(t) if t.kind == SyntaxKind::OpeningRoundBracket => {
                has_open = true;
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::ClosingRoundBracket => {
                has_close = true;
            }
            SyntaxChild::Tree(_) => {
                flush_line(&mut current_parts, &mut lines, ctx);
                current_parts.push(child);
            }
            _ => {
                current_parts.push(child);
            }
        }
    }
    flush_line(&mut current_parts, &mut lines, ctx);

    // Find max content width among lines that have comments
    let max_width = lines.iter()
        .filter(|l| l.comment.is_some())
        .map(|l| l.content.len())
        .max()
        .unwrap_or(0);

    // Emit
    if has_open {
        ctx.write_token("(");
        ctx.indent();
    }

    for line in &lines {
        if !ctx.is_at_line_start() {
            ctx.write_newline();
        }
        ctx.write_token(&line.content);
        if let Some(ref comment) = line.comment {
            let pad = max_width.saturating_sub(line.content.len());
            ctx.write_padding(pad);
            ctx.write_space();
            ctx.write_token(comment);
            if comment.starts_with("--") {
                ctx.write_newline();
            }
        }
    }

    if has_close {
        ctx.dedent();
        if !ctx.is_at_line_start() {
            ctx.write_newline();
        }
        ctx.write_token(")");
    }
}

// ---------------------------------------------------------------------------
// ALTER statement
// ---------------------------------------------------------------------------

fn format_alter_statement(tree: &SyntaxTree, ctx: &mut FormatterContext) {
    let mut need_sep = false;
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                if need_sep {
                    ctx.write_space();
                }
                ctx.write_keyword(t.text(ctx.source));
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
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
                ctx.write_token(",");
                ctx.write_newline();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
                ctx.write_newline();
                ctx.indent();
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Comma => {
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
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                if after_first_select {
                    // This is the UNION/EXCEPT/INTERSECT keyword or ALL/DISTINCT
                    ctx.write_space();
                    ctx.write_keyword(t.text(ctx.source));
                } else {
                    ctx.write_keyword(t.text(ctx.source));
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
    for child in &tree.children {
        match child {
            SyntaxChild::Token(t) if t.kind == SyntaxKind::Whitespace => {
                ctx.note_skipped_whitespace(t.text(ctx.source));
            }
            SyntaxChild::Token(t) if t.kind == SyntaxKind::BareWord => {
                ctx.write_keyword(t.text(ctx.source));
                ctx.write_space();
            }
            SyntaxChild::Token(t) => emit_token(t, ctx),
            SyntaxChild::Tree(subtree) if subtree.kind == SyntaxKind::ExplainKind => {
                format_node(subtree, ctx);
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
