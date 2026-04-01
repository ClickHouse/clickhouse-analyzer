use crate::lexer::token::TokenKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Parse optional PARTITION expr, wrapping in PartitionExpression.
fn parse_partition(p: &mut Parser) {
    if p.at_keyword(Keyword::Partition) {
        let m = p.start();
        p.expect_keyword(Keyword::Partition);
        parse_expression(p);
        p.complete(m, SyntaxKind::PartitionExpression);
    }
}

// ---------------------------------------------------------------------------
// USE statement
// ---------------------------------------------------------------------------

pub fn at_use_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Use)
}

pub fn parse_use_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Use);

    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected database name after USE");
    }

    p.complete(m, SyntaxKind::UseStatement);
}

// ---------------------------------------------------------------------------
// SET statement
// ---------------------------------------------------------------------------

pub fn at_set_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Set)
}

pub fn parse_set_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Set);

    let mut first = true;
    while !p.end_of_statement() {
        if !first {
            p.expect(TokenKind::Comma);
        }
        first = false;

        common::parse_setting_item(p);
    }

    p.complete(m, SyntaxKind::SetStatement);
}

// ---------------------------------------------------------------------------
// DROP statement
// ---------------------------------------------------------------------------

pub fn at_drop_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Drop)
}

pub fn parse_drop_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Drop);

    // Optional TEMPORARY
    let _ = p.eat_keyword(Keyword::Temporary);

    // Object kind: TABLE, DATABASE, VIEW, DICTIONARY, FUNCTION
    // TABLE is optional for DROP TABLE
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    } else if p.at_keyword(Keyword::View) {
        p.advance();
    } else if p.at_keyword(Keyword::Dictionary) {
        p.advance();
    } else if p.at_keyword(Keyword::Function) {
        p.advance();
    }
    // If none matched, that's ok -- DROP [IF EXISTS] name is valid shorthand

    common::parse_if_exists(p);

    // Parse the identifier
    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    // Optional PERMANENTLY
    let _ = p.eat_keyword(Keyword::Permanently);

    // Optional SYNC
    let _ = p.eat_keyword(Keyword::Sync);

    p.complete(m, SyntaxKind::DropStatement);
}

// ---------------------------------------------------------------------------
// TRUNCATE statement
// ---------------------------------------------------------------------------

pub fn at_truncate_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Truncate)
}

pub fn parse_truncate_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Truncate);

    // Optional TABLE keyword
    let _ = p.eat_keyword(Keyword::Table);

    common::parse_if_exists(p);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    // Optional SYNC
    let _ = p.eat_keyword(Keyword::Sync);

    p.complete(m, SyntaxKind::TruncateStatement);
}

// ---------------------------------------------------------------------------
// RENAME statement
// ---------------------------------------------------------------------------

pub fn at_rename_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Rename)
}

pub fn parse_rename_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Rename);
    p.expect_keyword(Keyword::Table);

    let mut first = true;
    loop {
        if !first {
            if !p.at(TokenKind::Comma) {
                break;
            }
            p.expect(TokenKind::Comma);
        }
        first = false;

        parse_rename_item(p);

        if p.end_of_statement() || p.at_keyword(Keyword::On) {
            break;
        }
    }

    common::parse_on_cluster(p);

    p.complete(m, SyntaxKind::RenameStatement);
}

fn parse_rename_item(p: &mut Parser) {
    let m = p.start();

    common::parse_table_identifier(p);

    if p.at_keyword(Keyword::To) {
        p.expect_keyword(Keyword::To);
    } else {
        p.recover_with_error("Expected TO in RENAME");
    }

    common::parse_table_identifier(p);

    p.complete(m, SyntaxKind::RenameItem);
}

// ---------------------------------------------------------------------------
// EXISTS statement
// ---------------------------------------------------------------------------

pub fn at_exists_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Exists)
}

pub fn parse_exists_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Exists);

    // Optional TEMPORARY
    let _ = p.eat_keyword(Keyword::Temporary);

    // Optional object type keyword
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    } else if p.at_keyword(Keyword::View) {
        p.advance();
    } else if p.at_keyword(Keyword::Dictionary) {
        p.advance();
    }

    common::parse_table_identifier(p);

    p.complete(m, SyntaxKind::ExistsStatement);
}

// ---------------------------------------------------------------------------
// CHECK statement
// ---------------------------------------------------------------------------

pub fn at_check_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Check)
}

pub fn parse_check_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Check);
    p.expect_keyword(Keyword::Table);

    common::parse_table_identifier(p);

    parse_partition(p);

    p.complete(m, SyntaxKind::CheckStatement);
}

// ---------------------------------------------------------------------------
// OPTIMIZE statement
// ---------------------------------------------------------------------------

pub fn at_optimize_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Optimize)
}

pub fn parse_optimize_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Optimize);
    p.expect_keyword(Keyword::Table);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    parse_partition(p);

    // Optional FINAL
    let _ = p.eat_keyword(Keyword::Final);

    // Optional DEDUPLICATE [BY expr, ...]
    if p.at_keyword(Keyword::Deduplicate) {
        p.advance();

        if p.at_keyword(Keyword::By) {
            p.advance();
            // Parse comma-separated expression list
            let m = p.start();
            parse_expression(p);
            while p.at(TokenKind::Comma) && !p.end_of_statement() {
                p.advance();
                parse_expression(p);
            }
            p.complete(m, SyntaxKind::IdentifierList);
        }
    }

    p.complete(m, SyntaxKind::OptimizeStatement);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    fn check(input: &str, expected_tree: Expect) {
        let result = parse(input);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        expected_tree.assert_eq(&buf);
    }

    #[test]
    fn test_use_statement() {
        check(
            "USE mydb",
            expect![[r#"
                File
                  UseStatement
                    'USE'
                    'mydb'
            "#]],
        );
    }

    #[test]
    fn test_use_statement_missing_db() {
        check(
            "USE",
            expect![[r#"
                File
                  UseStatement
                    'USE'
                    Error
            "#]],
        );
    }

    #[test]
    fn test_set_single() {
        check(
            "SET max_threads = 4",
            expect![[r#"
                File
                  SetStatement
                    'SET'
                    SettingItem
                      'max_threads'
                      '='
                      NumberLiteral
                        '4'
            "#]],
        );
    }

    #[test]
    fn test_set_multiple() {
        check(
            "SET max_threads = 4, max_memory_usage = 1000000",
            expect![[r#"
                File
                  SetStatement
                    'SET'
                    SettingItem
                      'max_threads'
                      '='
                      NumberLiteral
                        '4'
                    ','
                    SettingItem
                      'max_memory_usage'
                      '='
                      NumberLiteral
                        '1000000'
            "#]],
        );
    }

    #[test]
    fn test_drop_table() {
        check(
            "DROP TABLE mydb.mytable",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_drop_table_if_exists() {
        check(
            "DROP TABLE IF EXISTS mydb.mytable",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_drop_database_on_cluster() {
        check(
            "DROP DATABASE IF EXISTS mydb ON CLUSTER mycluster",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'DATABASE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'mydb'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_drop_temporary_table() {
        check(
            "DROP TEMPORARY TABLE tmp",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TEMPORARY'
                    'TABLE'
                    TableIdentifier
                      'tmp'
            "#]],
        );
    }

    #[test]
    fn test_drop_view() {
        check(
            "DROP VIEW IF EXISTS myview",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'VIEW'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'myview'
            "#]],
        );
    }

    #[test]
    fn test_drop_permanently() {
        check(
            "DROP TABLE mytable PERMANENTLY",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'PERMANENTLY'
            "#]],
        );
    }

    #[test]
    fn test_drop_sync() {
        check(
            "DROP TABLE mytable SYNC",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'SYNC'
            "#]],
        );
    }

    #[test]
    fn test_truncate_table() {
        check(
            "TRUNCATE TABLE mydb.mytable",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_truncate_without_table_keyword() {
        check(
            "TRUNCATE mydb.mytable",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_truncate_if_exists_on_cluster() {
        check(
            "TRUNCATE TABLE IF EXISTS mytable ON CLUSTER mycluster",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    'TABLE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'mytable'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_truncate_sync() {
        check(
            "TRUNCATE TABLE default.records SYNC",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    'TABLE'
                    TableIdentifier
                      'default'
                      '.'
                      'records'
                    'SYNC'
            "#]],
        );
    }

    #[test]
    fn test_rename_table() {
        check(
            "RENAME TABLE old TO new",
            expect![[r#"
                File
                  RenameStatement
                    'RENAME'
                    'TABLE'
                    RenameItem
                      TableIdentifier
                        'old'
                      'TO'
                      TableIdentifier
                        'new'
            "#]],
        );
    }

    #[test]
    fn test_rename_multiple() {
        check(
            "RENAME TABLE db.old1 TO db.new1, db.old2 TO db.new2",
            expect![[r#"
                File
                  RenameStatement
                    'RENAME'
                    'TABLE'
                    RenameItem
                      TableIdentifier
                        'db'
                        '.'
                        'old1'
                      'TO'
                      TableIdentifier
                        'db'
                        '.'
                        'new1'
                    ','
                    RenameItem
                      TableIdentifier
                        'db'
                        '.'
                        'old2'
                      'TO'
                      TableIdentifier
                        'db'
                        '.'
                        'new2'
            "#]],
        );
    }

    #[test]
    fn test_rename_on_cluster() {
        check(
            "RENAME TABLE old TO new ON CLUSTER mycluster",
            expect![[r#"
                File
                  RenameStatement
                    'RENAME'
                    'TABLE'
                    RenameItem
                      TableIdentifier
                        'old'
                      'TO'
                      TableIdentifier
                        'new'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_exists_table() {
        check(
            "EXISTS TABLE mydb.mytable",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_exists_database() {
        check(
            "EXISTS DATABASE mydb",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    'DATABASE'
                    TableIdentifier
                      'mydb'
            "#]],
        );
    }

    #[test]
    fn test_exists_no_keyword() {
        check(
            "EXISTS mytable",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    TableIdentifier
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_exists_temporary() {
        check(
            "EXISTS TEMPORARY TABLE tmp",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    'TEMPORARY'
                    'TABLE'
                    TableIdentifier
                      'tmp'
            "#]],
        );
    }

    #[test]
    fn test_check_table() {
        check(
            "CHECK TABLE mydb.mytable",
            expect![[r#"
                File
                  CheckStatement
                    'CHECK'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_check_table_partition() {
        check(
            "CHECK TABLE mytable PARTITION 202301",
            expect![[r#"
                File
                  CheckStatement
                    'CHECK'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    PartitionExpression
                      'PARTITION'
                      NumberLiteral
                        '202301'
            "#]],
        );
    }

    #[test]
    fn test_optimize_table() {
        check(
            "OPTIMIZE TABLE mydb.mytable",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_optimize_final() {
        check(
            "OPTIMIZE TABLE mytable FINAL",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'FINAL'
            "#]],
        );
    }

    #[test]
    fn test_optimize_deduplicate() {
        check(
            "OPTIMIZE TABLE mytable DEDUPLICATE",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'DEDUPLICATE'
            "#]],
        );
    }

    #[test]
    fn test_optimize_full() {
        check(
            "OPTIMIZE TABLE mytable ON CLUSTER mycluster PARTITION 202301 FINAL DEDUPLICATE",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
                    PartitionExpression
                      'PARTITION'
                      NumberLiteral
                        '202301'
                    'FINAL'
                    'DEDUPLICATE'
            "#]],
        );
    }

    #[test]
    fn test_set_string_value() {
        check(
            "SET log_comment = 'my test'",
            expect![[r#"
                File
                  SetStatement
                    'SET'
                    SettingItem
                      'log_comment'
                      '='
                      StringLiteral
                        ''my test''
            "#]],
        );
    }

    #[test]
    fn test_drop_function() {
        check(
            "DROP FUNCTION IF EXISTS my_func ON CLUSTER mycluster",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'FUNCTION'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'my_func'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_drop_dictionary() {
        check(
            "DROP DICTIONARY mydict",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'DICTIONARY'
                    TableIdentifier
                      'mydict'
            "#]],
        );
    }
}
