use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::grammar::select::{at_select_statement, parse_select_statement};
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;

const SHOW_KEYWORDS: &[Keyword] = &[
    Keyword::From, Keyword::Like, Keyword::Ilike, Keyword::Limit,
    Keyword::Format, Keyword::Where,
];

// ===========================================================================
// EXPLAIN statement
// ===========================================================================

pub fn at_explain_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Explain)
}

/// Parses: EXPLAIN [AST|SYNTAX|PLAN|PIPELINE|ESTIMATE|QUERY TREE|TABLE OVERRIDE]
///         [setting = value, ...] statement
pub fn parse_explain_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Explain);

    // Optional explain kind
    if at_explain_kind(p) {
        parse_explain_kind(p);
    }

    // Optional settings: SETTINGS key = value, ... OR bare key = value, ...
    if p.at_keyword(Keyword::Settings) {
        parse_settings_list(p);
    } else if at_inline_setting(p) {
        parse_inline_settings(p);
    }

    // Inner statement
    if at_select_statement(p) {
        parse_select_statement(p);
    } else if at_explain_statement(p) {
        parse_explain_statement(p);
    } else if at_show_statement(p) {
        parse_show_statement(p);
    } else if at_describe_statement(p) {
        parse_describe_statement(p);
    } else if !p.eof() && !p.end_of_statement() {
        // Try to consume the rest as an expression (for unknown statement types)
        p.advance_with_error("Expected statement after EXPLAIN");
        // Consume remaining tokens until end of statement
        while !p.eof() && !p.end_of_statement() {
            p.advance();
        }
    } else {
        p.recover_with_error("Expected statement after EXPLAIN");
    }

    p.complete(m, SyntaxKind::ExplainStatement);
}

fn at_explain_kind(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Ast)
        || p.at_keyword(Keyword::Syntax)
        || p.at_keyword(Keyword::Plan)
        || p.at_keyword(Keyword::Pipeline)
        || p.at_keyword(Keyword::Estimate)
        || p.at_keyword(Keyword::Query)
        || p.at_keyword(Keyword::Table)
}

fn parse_explain_kind(p: &mut Parser) {
    let m = p.start();

    if p.at_keyword(Keyword::Query) {
        // QUERY TREE
        p.advance();
        if p.at_keyword(Keyword::Tree) {
            p.advance();
        } else {
            p.recover_with_error("Expected TREE after QUERY");
        }
    } else if p.at_keyword(Keyword::Table) {
        // TABLE OVERRIDE
        p.advance();
        if p.at_keyword(Keyword::Override) {
            p.advance();
        } else {
            p.recover_with_error("Expected OVERRIDE after TABLE");
        }
    } else {
        // AST | SYNTAX | PLAN | PIPELINE | ESTIMATE
        p.advance();
    }

    p.complete(m, SyntaxKind::ExplainKind);
}

// ===========================================================================
// DESCRIBE / DESC statement
// ===========================================================================

pub fn at_describe_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Describe)
        || p.at_keyword(Keyword::Desc)
}

/// Parses: DESCRIBE|DESC [TABLE] [db.]table [FORMAT format]
///         DESCRIBE|DESC table_function(args) [FORMAT format]
pub fn parse_describe_statement(p: &mut Parser) {
    let m = p.start();

    // DESCRIBE or DESC
    if p.at_keyword(Keyword::Describe) {
        p.advance();
    } else if p.at_keyword(Keyword::Desc) {
        p.advance();
    }

    // Parenthesized subquery: DESCRIBE (SELECT ...)
    if p.at(SyntaxKind::OpeningRoundBracket) {
        let sm = p.start();
        p.advance(); // consume (
        if at_select_statement(p) {
            parse_select_statement(p);
        } else {
            p.recover_with_error("Expected SELECT inside parentheses");
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
        p.complete(sm, SyntaxKind::SubqueryExpression);
    } else {
        // Optional TABLE keyword
        p.eat_keyword(Keyword::Table);

        // Table identifier or table function
        if p.at_identifier() {
            parse_table_ref(p);
        } else if !p.eof() && !p.end_of_statement() {
            p.advance_with_error("Expected table name");
        } else {
            p.recover_with_error("Expected table name after DESCRIBE");
        }
    }

    // Optional FORMAT clause
    if p.at_keyword(Keyword::Format) {
        parse_format_clause(p);
    }

    p.complete(m, SyntaxKind::DescribeStatement);
}

// ===========================================================================
// SHOW statement
// ===========================================================================

pub fn at_show_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Show)
}

/// Parses all SHOW variants by dispatching on the word after SHOW.
pub fn parse_show_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Show);

    if p.at_keyword(Keyword::Tables) {
        parse_show_tables(p);
    } else if p.at_keyword(Keyword::Databases) {
        parse_show_databases(p);
    } else if p.at_keyword(Keyword::Create) {
        parse_show_create(p);
    } else if p.at_keyword(Keyword::Columns) {
        parse_show_columns(p);
    } else if p.at_keyword(Keyword::Processlist) {
        parse_show_processlist(p);
    } else if p.at_keyword(Keyword::Dictionaries) {
        parse_show_dictionaries(p);
    } else if p.at_keyword(Keyword::Functions) {
        parse_show_functions(p);
    } else if p.at_keyword(Keyword::Grants) {
        parse_show_grants(p);
    } else if p.at_keyword(Keyword::Privileges) {
        parse_show_privileges(p);
    } else if p.at_keyword(Keyword::Engines) {
        parse_show_engines(p);
    } else if p.at_keyword(Keyword::Settings) {
        parse_show_settings(p);
    } else if !p.eof() && !p.end_of_statement() {
        // Unknown SHOW target -- wrap in error and consume
        p.advance_with_error("Unknown SHOW target");
        while !p.eof() && !p.end_of_statement() {
            p.advance();
        }
    } else {
        p.recover_with_error("Expected target after SHOW");
    }

    p.complete(m, SyntaxKind::ShowStatement);
}

// ---------------------------------------------------------------------------
// SHOW sub-parsers
// ---------------------------------------------------------------------------

/// SHOW TABLES [FROM db] [LIKE 'pattern' | ILIKE 'pattern'] [LIMIT n]
fn parse_show_tables(p: &mut Parser) {
    let m = p.start();
    p.advance(); // TABLES

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::From) {
        parse_from_database(p);
    }

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Like) || p.at_keyword(Keyword::Ilike) {
        parse_like_clause(p);
    }

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Limit) {
        parse_optional_limit(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW DATABASES [LIKE 'pattern' | ILIKE 'pattern'] [LIMIT n]
fn parse_show_databases(p: &mut Parser) {
    let m = p.start();
    p.advance(); // DATABASES

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Like) || p.at_keyword(Keyword::Ilike) {
        parse_like_clause(p);
    }

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Limit) {
        parse_optional_limit(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW CREATE [TABLE|VIEW|DATABASE|DICTIONARY] [db.]name [FORMAT format]
fn parse_show_create(p: &mut Parser) {
    let m = p.start();
    p.advance(); // CREATE

    // Optional object type
    if p.at_keyword(Keyword::Table)
        || p.at_keyword(Keyword::View)
        || p.at_keyword(Keyword::Database)
        || p.at_keyword(Keyword::Dictionary)
        || p.at_keyword(Keyword::User)
        || p.at_keyword(Keyword::Role)
        || p.at_keyword(Keyword::Quota)
        || p.at_keyword(Keyword::Policy)
        || p.at_keyword(Keyword::Profile)
    {
        p.advance();
    } else if p.at_keyword(Keyword::Row) {
        p.advance(); // ROW
        if p.at_keyword(Keyword::Policy) {
            p.advance(); // POLICY
        }
    } else if p.at_keyword(Keyword::Settings) {
        p.advance(); // SETTINGS
        if p.at_keyword(Keyword::Profile) {
            p.advance(); // PROFILE
        }
    }

    // Object name: [db.]name
    if p.at_identifier() {
        parse_table_ref(p);
    }

    // Consume remaining tokens (e.g. ON table for ROW POLICY)
    while !p.eof() && !p.end_of_statement() && !p.at_keyword(Keyword::Format) {
        p.advance();
    }

    if p.at_keyword(Keyword::Format) {
        parse_format_clause(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW COLUMNS FROM [db.]table [LIKE 'pattern'] [LIMIT n]
fn parse_show_columns(p: &mut Parser) {
    let m = p.start();
    p.advance(); // COLUMNS

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::From) {
        p.advance(); // FROM
        if p.at_identifier() {
            parse_table_ref(p);
        } else {
            p.recover_with_error("Expected table name after FROM");
        }
    }

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Like) || p.at_keyword(Keyword::Ilike) {
        parse_like_clause(p);
    }

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Limit) {
        parse_optional_limit(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW PROCESSLIST [FORMAT format]
fn parse_show_processlist(p: &mut Parser) {
    let m = p.start();
    p.advance(); // PROCESSLIST

    if p.at_keyword(Keyword::Format) {
        parse_format_clause(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW DICTIONARIES [FROM db] [LIKE 'pattern']
fn parse_show_dictionaries(p: &mut Parser) {
    let m = p.start();
    p.advance(); // DICTIONARIES

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::From) {
        parse_from_database(p);
    }

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Like) || p.at_keyword(Keyword::Ilike) {
        parse_like_clause(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW FUNCTIONS [LIKE 'pattern' | ILIKE 'pattern']
fn parse_show_functions(p: &mut Parser) {
    let m = p.start();
    p.advance(); // FUNCTIONS

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Like) || p.at_keyword(Keyword::Ilike) {
        parse_like_clause(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW GRANTS [FOR user]
fn parse_show_grants(p: &mut Parser) {
    let m = p.start();
    p.advance(); // GRANTS

    if p.at_keyword(Keyword::For) {
        p.advance(); // FOR
        if p.at_identifier() {
            p.advance(); // user name
        } else {
            p.recover_with_error("Expected user name after FOR");
        }
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW PRIVILEGES
fn parse_show_privileges(p: &mut Parser) {
    let m = p.start();
    p.advance(); // PRIVILEGES
    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW ENGINES
fn parse_show_engines(p: &mut Parser) {
    let m = p.start();
    p.advance(); // ENGINES
    p.complete(m, SyntaxKind::ShowTarget);
}

/// SHOW SETTINGS [LIKE 'pattern']
fn parse_show_settings(p: &mut Parser) {
    let m = p.start();
    p.advance(); // SETTINGS

    common::skip_to_keywords(p, SHOW_KEYWORDS);

    if p.at_keyword(Keyword::Like) || p.at_keyword(Keyword::Ilike) {
        parse_like_clause(p);
    }

    p.complete(m, SyntaxKind::ShowTarget);
}

// ===========================================================================
// Helpers
// ===========================================================================

/// LIKE 'pattern' | ILIKE 'pattern'
fn parse_like_clause(p: &mut Parser) {
    let m = p.start();

    if p.at_keyword(Keyword::Like) {
        p.advance();
    } else if p.at_keyword(Keyword::Ilike) {
        p.advance();
    }

    if p.at(SyntaxKind::StringToken) {
        p.advance();
    } else {
        p.recover_with_error("Expected string pattern after LIKE/ILIKE");
    }

    p.complete(m, SyntaxKind::LikeClause);
}

/// FORMAT format_name
fn parse_format_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Format);

    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected format name after FORMAT");
    }

    p.complete(m, SyntaxKind::FormatClause);
}

/// FROM db_name (used in SHOW TABLES FROM db, SHOW DICTIONARIES FROM db)
fn parse_from_database(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::From);

    if p.at_identifier() {
        p.advance();
    } else if common::at_query_parameter(p) {
        common::parse_query_parameter(p);
    } else {
        p.recover_with_error("Expected database name after FROM");
    }

    p.complete(m, SyntaxKind::FromDatabaseClause);
}

/// LIMIT n -- simple version for SHOW statements
fn parse_optional_limit(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Limit);
    parse_expression(p);
    p.complete(m, SyntaxKind::LimitClause);
}

/// Parses a table reference: [db.]table or table_function(args)
fn parse_table_ref(p: &mut Parser) {
    let m = p.start();

    // First identifier
    p.advance();

    // Check for db.table syntax
    if p.at(SyntaxKind::Dot) {
        p.advance(); // consume dot
        if p.at_identifier() {
            p.advance();
        } else {
            p.recover_with_error("Expected table name after dot");
        }
        p.complete(m, SyntaxKind::TableIdentifier);
    } else if p.at(SyntaxKind::OpeningRoundBracket) {
        // table_function(args)
        p.advance(); // (
        while !p.eof() && !p.at(SyntaxKind::ClosingRoundBracket) {
            if p.at(SyntaxKind::Comma) {
                p.advance();
            } else {
                parse_expression(p);
            }
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
        p.complete(m, SyntaxKind::TableFunction);
    } else {
        p.complete(m, SyntaxKind::TableIdentifier);
    }
}

/// True if the parser is at an inline setting: `identifier =` (not preceded by SETTINGS keyword).
/// Used for EXPLAIN QUERY TREE which accepts bare settings before the inner statement.
fn at_inline_setting(p: &mut Parser) -> bool {
    p.at_identifier()
        && p.nth(1) == SyntaxKind::Equals
        && !at_select_statement(p)
        && !at_explain_statement(p)
        && !at_show_statement(p)
        && !at_describe_statement(p)
}

/// Parse inline settings (without SETTINGS keyword): key = value [, key = value, ...]
fn parse_inline_settings(p: &mut Parser) {
    let m = p.start();

    let mut first = true;
    while !p.eof()
        && !p.end_of_statement()
        && at_inline_setting(p)
    {
        if !first {
            if !p.eat(SyntaxKind::Comma) {
                break;
            }
        }
        first = false;

        let item_m = p.start();
        p.advance(); // key
        p.expect(SyntaxKind::Equals);
        parse_expression(p);
        p.complete(item_m, SyntaxKind::SettingItem);
    }

    p.complete(m, SyntaxKind::SettingsClause);
}

/// Parses: SETTINGS key = value, key = value, ...
fn parse_settings_list(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Settings);

    let mut first = true;
    while !p.eof()
        && !p.end_of_statement()
        && !at_select_statement(p)
        && !at_explain_statement(p)
        && !at_show_statement(p)
        && !at_describe_statement(p)
    {
        if !first {
            if !p.eat(SyntaxKind::Comma) {
                break;
            }
        }
        first = false;

        let item_m = p.start();
        // key
        if p.at_identifier() {
            p.advance();
        } else {
            break;
        }
        // =
        p.expect(SyntaxKind::Equals);
        // value
        parse_expression(p);
        p.complete(item_m, SyntaxKind::SettingItem);
    }

    p.complete(m, SyntaxKind::SettingsClause);
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    fn check(input: &str, expected: Expect) {
        let result = parse(input);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        expected.assert_eq(&buf);
    }

    fn check_no_errors(input: &str) {
        let result = parse(input);
        assert!(
            result.errors.is_empty(),
            "Expected no errors for `{input}`, got: {:?}",
            result.errors,
        );
    }

    fn check_roundtrip(input: &str) {
        let result = parse(input);
        let reconstructed = collect_text(&result.tree, &result.source);
        assert_eq!(
            reconstructed, input,
            "CST does not reconstruct original input"
        );
    }

    fn collect_text(tree: &crate::SyntaxTree, source: &str) -> String {
        let mut buf = String::new();
        collect_text_rec(tree, &mut buf, source);
        buf
    }

    fn collect_text_rec(tree: &crate::SyntaxTree, buf: &mut String, source: &str) {
        for child in &tree.children {
            match child {
                crate::SyntaxChild::Token(token) => buf.push_str(token.text(source)),
                crate::SyntaxChild::Tree(subtree) => collect_text_rec(subtree, buf, source),
            }
        }
    }

    // -----------------------------------------------------------------------
    // EXPLAIN
    // -----------------------------------------------------------------------

    #[test]
    fn explain_ast_select() {
        check(
            "EXPLAIN AST SELECT 1",
            expect![[r#"
                File
                  ExplainStatement
                    'EXPLAIN'
                    ExplainKind
                      'AST'
                    SelectStatement
                      SelectClause
                        'SELECT'
                        ColumnList
                          NumberLiteral
                            '1'
            "#]],
        );
    }

    #[test]
    fn explain_plan_select() {
        check_no_errors("EXPLAIN PLAN SELECT 1 FROM t");
        check_roundtrip("EXPLAIN PLAN SELECT 1 FROM t");
    }

    #[test]
    fn explain_pipeline_select() {
        check_no_errors("EXPLAIN PIPELINE SELECT count() FROM t");
        check_roundtrip("EXPLAIN PIPELINE SELECT count() FROM t");
    }

    #[test]
    fn explain_estimate_select() {
        check_no_errors("EXPLAIN ESTIMATE SELECT 1 FROM t");
        check_roundtrip("EXPLAIN ESTIMATE SELECT 1 FROM t");
    }

    #[test]
    fn explain_syntax_select() {
        check_no_errors("EXPLAIN SYNTAX SELECT 1");
        check_roundtrip("EXPLAIN SYNTAX SELECT 1");
    }

    #[test]
    fn explain_query_tree_select() {
        check(
            "EXPLAIN QUERY TREE SELECT 1",
            expect![[r#"
                File
                  ExplainStatement
                    'EXPLAIN'
                    ExplainKind
                      'QUERY'
                      'TREE'
                    SelectStatement
                      SelectClause
                        'SELECT'
                        ColumnList
                          NumberLiteral
                            '1'
            "#]],
        );
    }

    #[test]
    fn explain_table_override() {
        check_no_errors("EXPLAIN TABLE OVERRIDE SELECT 1");
    }

    #[test]
    fn explain_no_kind() {
        check(
            "EXPLAIN SELECT 1",
            expect![[r#"
                File
                  ExplainStatement
                    'EXPLAIN'
                    SelectStatement
                      SelectClause
                        'SELECT'
                        ColumnList
                          NumberLiteral
                            '1'
            "#]],
        );
    }

    #[test]
    fn explain_without_statement() {
        // Should produce error but valid tree
        let result = parse("EXPLAIN AST");
        assert!(!result.errors.is_empty());
        check_roundtrip("EXPLAIN AST");
    }

    #[test]
    fn explain_lowercase() {
        check_no_errors("explain ast select 1");
        check_roundtrip("explain ast select 1");
    }

    // -----------------------------------------------------------------------
    // DESCRIBE / DESC
    // -----------------------------------------------------------------------

    #[test]
    fn describe_table() {
        check(
            "DESCRIBE TABLE my_table",
            expect![[r#"
                File
                  DescribeStatement
                    'DESCRIBE'
                    'TABLE'
                    TableIdentifier
                      'my_table'
            "#]],
        );
    }

    #[test]
    fn desc_table() {
        check(
            "DESC my_table",
            expect![[r#"
                File
                  DescribeStatement
                    'DESC'
                    TableIdentifier
                      'my_table'
            "#]],
        );
    }

    #[test]
    fn describe_db_dot_table() {
        check(
            "DESCRIBE TABLE mydb.my_table",
            expect![[r#"
                File
                  DescribeStatement
                    'DESCRIBE'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'my_table'
            "#]],
        );
    }

    #[test]
    fn describe_with_format() {
        check(
            "DESCRIBE TABLE t FORMAT JSON",
            expect![[r#"
                File
                  DescribeStatement
                    'DESCRIBE'
                    'TABLE'
                    TableIdentifier
                      't'
                    FormatClause
                      'FORMAT'
                      'JSON'
            "#]],
        );
    }

    #[test]
    fn describe_table_function() {
        check(
            "DESCRIBE numbers(10)",
            expect![[r#"
                File
                  DescribeStatement
                    'DESCRIBE'
                    TableFunction
                      'numbers'
                      '('
                      NumberLiteral
                        '10'
                      ')'
            "#]],
        );
    }

    #[test]
    fn describe_without_table_name() {
        let result = parse("DESCRIBE");
        assert!(!result.errors.is_empty());
        check_roundtrip("DESCRIBE");
    }

    #[test]
    fn desc_lowercase() {
        check_no_errors("desc my_table");
        check_roundtrip("desc my_table");
    }

    // -----------------------------------------------------------------------
    // SHOW TABLES
    // -----------------------------------------------------------------------

    #[test]
    fn show_tables() {
        check(
            "SHOW TABLES",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'TABLES'
            "#]],
        );
    }

    #[test]
    fn show_tables_from_db() {
        check(
            "SHOW TABLES FROM mydb",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'TABLES'
                      FromDatabaseClause
                        'FROM'
                        'mydb'
            "#]],
        );
    }

    #[test]
    fn show_tables_like() {
        check(
            "SHOW TABLES LIKE '%test%'",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'TABLES'
                      LikeClause
                        'LIKE'
                        ''%test%''
            "#]],
        );
    }

    #[test]
    fn show_tables_ilike() {
        check_no_errors("SHOW TABLES ILIKE '%test%'");
        check_roundtrip("SHOW TABLES ILIKE '%test%'");
    }

    #[test]
    fn show_tables_from_db_like_limit() {
        check(
            "SHOW TABLES FROM mydb LIKE '%t%' LIMIT 10",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'TABLES'
                      FromDatabaseClause
                        'FROM'
                        'mydb'
                      LikeClause
                        'LIKE'
                        ''%t%''
                      LimitClause
                        'LIMIT'
                        NumberLiteral
                          '10'
            "#]],
        );
    }

    // -----------------------------------------------------------------------
    // SHOW DATABASES
    // -----------------------------------------------------------------------

    #[test]
    fn show_databases() {
        check(
            "SHOW DATABASES",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'DATABASES'
            "#]],
        );
    }

    #[test]
    fn show_databases_like() {
        check_no_errors("SHOW DATABASES LIKE '%test%'");
        check_roundtrip("SHOW DATABASES LIKE '%test%'");
    }

    // -----------------------------------------------------------------------
    // SHOW CREATE
    // -----------------------------------------------------------------------

    #[test]
    fn show_create_table() {
        check(
            "SHOW CREATE TABLE my_table",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'CREATE'
                      'TABLE'
                      TableIdentifier
                        'my_table'
            "#]],
        );
    }

    #[test]
    fn show_create_database() {
        check_no_errors("SHOW CREATE DATABASE mydb");
        check_roundtrip("SHOW CREATE DATABASE mydb");
    }

    #[test]
    fn show_create_db_dot_name() {
        check(
            "SHOW CREATE TABLE mydb.t",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'CREATE'
                      'TABLE'
                      TableIdentifier
                        'mydb'
                        '.'
                        't'
            "#]],
        );
    }

    #[test]
    fn show_create_with_format() {
        check_no_errors("SHOW CREATE TABLE t FORMAT JSON");
    }

    // -----------------------------------------------------------------------
    // SHOW COLUMNS
    // -----------------------------------------------------------------------

    #[test]
    fn show_columns_from() {
        check(
            "SHOW COLUMNS FROM my_table",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'COLUMNS'
                      'FROM'
                      TableIdentifier
                        'my_table'
            "#]],
        );
    }

    #[test]
    fn show_columns_like_limit() {
        check_no_errors("SHOW COLUMNS FROM t LIKE '%col%' LIMIT 5");
    }

    // -----------------------------------------------------------------------
    // SHOW PROCESSLIST
    // -----------------------------------------------------------------------

    #[test]
    fn show_processlist() {
        check(
            "SHOW PROCESSLIST",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'PROCESSLIST'
            "#]],
        );
    }

    #[test]
    fn show_processlist_format() {
        check_no_errors("SHOW PROCESSLIST FORMAT JSON");
    }

    // -----------------------------------------------------------------------
    // SHOW DICTIONARIES
    // -----------------------------------------------------------------------

    #[test]
    fn show_dictionaries() {
        check_no_errors("SHOW DICTIONARIES");
        check_roundtrip("SHOW DICTIONARIES");
    }

    #[test]
    fn show_dictionaries_from_like() {
        check_no_errors("SHOW DICTIONARIES FROM mydb LIKE '%dict%'");
    }

    // -----------------------------------------------------------------------
    // SHOW FUNCTIONS
    // -----------------------------------------------------------------------

    #[test]
    fn show_functions() {
        check_no_errors("SHOW FUNCTIONS");
        check_roundtrip("SHOW FUNCTIONS");
    }

    #[test]
    fn show_functions_ilike() {
        check_no_errors("SHOW FUNCTIONS ILIKE '%concat%'");
    }

    // -----------------------------------------------------------------------
    // SHOW GRANTS
    // -----------------------------------------------------------------------

    #[test]
    fn show_grants() {
        check(
            "SHOW GRANTS",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'GRANTS'
            "#]],
        );
    }

    #[test]
    fn show_grants_for_user() {
        check(
            "SHOW GRANTS FOR admin",
            expect![[r#"
                File
                  ShowStatement
                    'SHOW'
                    ShowTarget
                      'GRANTS'
                      'FOR'
                      'admin'
            "#]],
        );
    }

    // -----------------------------------------------------------------------
    // SHOW PRIVILEGES / ENGINES / SETTINGS
    // -----------------------------------------------------------------------

    #[test]
    fn show_privileges() {
        check_no_errors("SHOW PRIVILEGES");
    }

    #[test]
    fn show_engines() {
        check_no_errors("SHOW ENGINES");
    }

    #[test]
    fn show_settings_like() {
        check_no_errors("SHOW SETTINGS LIKE '%max%'");
    }

    // -----------------------------------------------------------------------
    // Error recovery
    // -----------------------------------------------------------------------

    #[test]
    fn show_without_target() {
        let result = parse("SHOW");
        assert!(!result.errors.is_empty());
        check_roundtrip("SHOW");
    }

    #[test]
    fn show_unknown_target() {
        let result = parse("SHOW FOOBAR");
        assert!(!result.errors.is_empty());
        check_roundtrip("SHOW FOOBAR");
    }

    // -----------------------------------------------------------------------
    // Case insensitivity
    // -----------------------------------------------------------------------

    #[test]
    fn show_tables_lowercase() {
        check_no_errors("show tables from mydb like '%t%'");
    }

    #[test]
    fn explain_mixed_case() {
        check_no_errors("Explain Ast Select 1");
    }

    // -----------------------------------------------------------------------
    // Roundtrip: every byte of input is preserved in CST
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_explain() {
        check_roundtrip("EXPLAIN AST SELECT a, b FROM t WHERE x > 1");
    }

    #[test]
    fn roundtrip_describe() {
        check_roundtrip("DESCRIBE TABLE mydb.my_table FORMAT JSON");
    }

    #[test]
    fn roundtrip_show() {
        check_roundtrip("SHOW TABLES FROM mydb LIKE '%t%' LIMIT 10");
    }
}
