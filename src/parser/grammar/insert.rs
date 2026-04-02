use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::grammar::select::{at_select_statement, parse_select_statement};
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;

const INSERT_KEYWORDS: &[Keyword] = &[
    Keyword::Values, Keyword::Format, Keyword::Settings,
    Keyword::Select, Keyword::With,
];

pub fn at_insert_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Insert)
}

pub fn parse_insert_statement(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Insert);
    p.expect_keyword(Keyword::Into);

    // Optional TABLE keyword
    p.eat_keyword(Keyword::Table);

    // Check for FUNCTION keyword (INSERT INTO FUNCTION s3(...) ...)
    if p.eat_keyword(Keyword::Function) {
        parse_table_function(p);
    } else {
        common::parse_table_identifier(p);
    }

    // Optional column list: (col1, col2, ...)
    if p.at(SyntaxKind::OpeningRoundBracket) {
        parse_insert_columns(p);
    }

    // Skip unexpected tokens before SETTINGS/VALUES/FORMAT/SELECT
    common::skip_to_keywords(p, INSERT_KEYWORDS);

    // Optional SETTINGS clause before data
    if p.at_keyword(Keyword::Settings) {
        parse_settings_clause(p);
    }

    // Skip unexpected tokens before data clause
    common::skip_to_keywords(p, INSERT_KEYWORDS);

    // Data part: VALUES, SELECT, or FORMAT
    if p.at_keyword(Keyword::Values) {
        parse_values_clause(p);
    } else if at_select_statement(p) {
        parse_select_statement(p);
    } else if p.at_keyword(Keyword::Format) {
        parse_format_clause(p);
    }
    // If none match, that's okay -- incomplete INSERT is still valid CST

    p.complete(m, SyntaxKind::InsertStatement);
}

// Parse a table function: func_name(args)
fn parse_table_function(p: &mut Parser) {
    let m = p.start();

    if p.at_identifier() {
        p.advance();
    } else {
        p.advance_with_error("Expected function name");
    }

    // Parse argument list
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.expect(SyntaxKind::OpeningRoundBracket);
        let mut first = true;
        while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;
            parse_expression(p);
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
    }

    p.complete(m, SyntaxKind::TableFunction);
}

// Parse optional column list: (col1, col2, ...)
fn parse_insert_columns(p: &mut Parser) {
    let m = p.start();

    p.expect(SyntaxKind::OpeningRoundBracket);

    let mut first = true;
    while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        if p.at_identifier() {
            p.advance();
        } else {
            p.advance_with_error("Expected column name");
        }
    }

    p.expect(SyntaxKind::ClosingRoundBracket);

    p.complete(m, SyntaxKind::InsertColumnsClause);
}

// Parse SETTINGS key=val, key=val
fn parse_settings_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Settings);

    let mut first = true;
    while !p.end_of_statement()
        && !p.at_keyword(Keyword::Values)
        && !p.at_keyword(Keyword::Format)
        && !at_select_statement(p)
    {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        common::parse_setting_item(p);
    }

    p.complete(m, SyntaxKind::SettingsClause);
}

// Parse VALUES (v1, v2), (v3, v4), ...
fn parse_values_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Values);

    // Parse value rows
    while p.at(SyntaxKind::OpeningRoundBracket) && !p.eof() {
        parse_value_row(p);

        // Optional comma between rows
        if p.at(SyntaxKind::Comma) {
            p.advance();
        }
    }

    p.complete(m, SyntaxKind::InsertValuesClause);
}

// Parse a single value row: (v1, v2, v3)
fn parse_value_row(p: &mut Parser) {
    let m = p.start();

    p.expect(SyntaxKind::OpeningRoundBracket);

    let mut first = true;
    while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        parse_expression(p);
    }

    p.expect(SyntaxKind::ClosingRoundBracket);

    p.complete(m, SyntaxKind::ValueRow);
}

// Parse FORMAT format_name
fn parse_format_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Format);

    if p.at_identifier() {
        p.advance();
    } else {
        p.advance_with_error("Expected format name");
    }

    p.complete(m, SyntaxKind::InsertFormatClause);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    fn check(input: &str, expected_tree: Expect) {
        let result = parse(input);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        expected_tree.assert_eq(&buf);
    }

    #[test]
    fn insert_values_basic() {
        check(
            "INSERT INTO t VALUES (1, 2, 3)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ','
                        NumberLiteral
                          '2'
                        ','
                        NumberLiteral
                          '3'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_values_multiple_rows() {
        check(
            "INSERT INTO t VALUES (1, 2), (3, 4)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ','
                        NumberLiteral
                          '2'
                        ')'
                      ','
                      ValueRow
                        '('
                        NumberLiteral
                          '3'
                        ','
                        NumberLiteral
                          '4'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_with_columns() {
        check(
            "INSERT INTO t (a, b) VALUES (1, 2)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    InsertColumnsClause
                      '('
                      'a'
                      ','
                      'b'
                      ')'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ','
                        NumberLiteral
                          '2'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_with_database_table() {
        check(
            "INSERT INTO db.t VALUES (1)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      'db'
                      '.'
                      't'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_select() {
        check(
            "INSERT INTO t SELECT 1, 2",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    SelectStatement
                      SelectClause
                        'SELECT'
                        ColumnList
                          NumberLiteral
                            '1'
                          ','
                          NumberLiteral
                            '2'
            "#]],
        );
    }

    #[test]
    fn insert_format() {
        check(
            "INSERT INTO t FORMAT JSONEachRow",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    InsertFormatClause
                      'FORMAT'
                      'JSONEachRow'
            "#]],
        );
    }

    #[test]
    fn insert_table_keyword() {
        check(
            "INSERT INTO TABLE t VALUES (1)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    'TABLE'
                    TableIdentifier
                      't'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_function() {
        check(
            "INSERT INTO FUNCTION s3('url') VALUES (1)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    'FUNCTION'
                    TableFunction
                      's3'
                      '('
                      StringLiteral
                        ''url''
                      ')'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_settings() {
        check(
            "INSERT INTO t SETTINGS async_insert = 1 VALUES (1)",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    SettingsClause
                      'SETTINGS'
                      SettingItem
                        'async_insert'
                        '='
                        NumberLiteral
                          '1'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        NumberLiteral
                          '1'
                        ')'
            "#]],
        );
    }

    #[test]
    fn insert_string_values() {
        check(
            "INSERT INTO t VALUES ('hello', 'world')",
            expect![[r#"
                File
                  InsertStatement
                    'INSERT'
                    'INTO'
                    TableIdentifier
                      't'
                    InsertValuesClause
                      'VALUES'
                      ValueRow
                        '('
                        StringLiteral
                          ''hello''
                        ','
                        StringLiteral
                          ''world''
                        ')'
            "#]],
        );
    }
}
