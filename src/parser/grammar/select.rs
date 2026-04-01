use crate::lexer::token::TokenKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Parses a full SELECT statement:
///   [WITH ...] [FROM ...] SELECT ... [FROM ...] [WHERE ...] [ORDER BY ...] [LIMIT ...]
///
/// ClickHouse allows FROM before SELECT.
pub fn parse_select_statement(p: &mut Parser) {
    let m = p.start();

    if p.at_keyword(Keyword::With) {
        parse_with_clause(p);
    }

    let mut parsed_early_from = false;
    if p.at_keyword(Keyword::From) {
        parse_from_clause(p);
        parsed_early_from = true;
    }

    parse_select_clause(p);

    if p.at_keyword(Keyword::From) {
        parse_from_clause(p);

        if parsed_early_from {
            p.recover_with_error("Duplicate FROM clause");
        }
    }

    skip_to_clause_keyword(p);

    if p.at_keyword(Keyword::Where) {
        let m = p.start();
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
        p.complete(m, SyntaxKind::WhereClause);
    }

    skip_to_clause_keyword(p);

    if p.at_keyword(Keyword::Order) {
        let m = p.start();
        p.expect_keyword(Keyword::Order);
        p.expect_keyword(Keyword::By);
        let m2 = p.start();
        parse_expression(p);
        p.complete(m2, SyntaxKind::OrderByItem);
        p.complete(m, SyntaxKind::OrderByClause);
    }

    skip_to_clause_keyword(p);

    if p.at_keyword(Keyword::Limit) {
        let m = p.start();
        p.expect_keyword(Keyword::Limit);
        parse_expression(p);
        p.complete(m, SyntaxKind::LimitClause);
    }

    p.complete(m, SyntaxKind::SelectStatement);
}

/// True if the current position marks the end of a column list
/// (i.e. we've hit a clause keyword or statement boundary).
pub fn at_end_of_column_list(p: &mut Parser) -> bool {
    at_clause_keyword(p)
}

/// True if the parser is positioned at a clause keyword that can appear
/// inside a SELECT statement. Used for error recovery.
fn at_clause_keyword(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Select)
        || p.at_keyword(Keyword::From)
        || p.at_keyword(Keyword::Where)
        || p.at_keyword(Keyword::Order)
        || p.at_keyword(Keyword::Limit)
}

/// True if the parser is positioned at the start of a SELECT statement.
pub fn at_select_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::With) || p.at_keyword(Keyword::Select) || p.at_keyword(Keyword::From)
}

/// Skips unexpected tokens until we reach a clause keyword or end of statement.
/// Wraps each skipped token in an Error node.
fn skip_to_clause_keyword(p: &mut Parser) {
    while !p.eof() && !p.end_of_statement() && !at_clause_keyword(p) {
        p.advance_with_error("Unexpected token");
    }
}

/// Parses: WITH expr [, expr ...]
fn parse_with_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::With);
    parse_column_list(p);
    p.complete(m, SyntaxKind::WithClause);
}

/// Parses: SELECT expr [, expr ...]
fn parse_select_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Select);
    parse_column_list(p);
    p.complete(m, SyntaxKind::SelectClause);
}

/// Parses a comma-separated list of expressions with optional aliases.
///   expr [AS alias | alias], expr [AS alias | alias], ...
pub fn parse_column_list(p: &mut Parser) {
    let m = p.start();

    let mut first = true;
    while !at_end_of_column_list(p) && !p.end_of_statement() {
        if !first {
            p.expect(TokenKind::Comma);
        }
        first = false;

        parse_expression(p);

        if p.at_keyword(Keyword::As)
            || (!at_end_of_column_list(p) && p.at(TokenKind::BareWord))
            || p.at(TokenKind::QuotedIdentifier)
        {
            let m = p.start();
            if p.at_keyword(Keyword::As) {
                p.expect_keyword(Keyword::As);
            }

            if !at_end_of_column_list(p) {
                p.advance()
            } else {
                p.recover_with_error("Expected column alias");
            }

            p.complete(m, SyntaxKind::ColumnAlias);
        }
    }

    p.complete(m, SyntaxKind::ColumnList);
}

/// Parses: FROM table_reference
fn parse_from_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::From);
    parse_table_reference(p);
    p.complete(m, SyntaxKind::FromClause);
}

/// Parses a table reference: identifier [. identifier]
///   e.g. `system.numbers`, `"my_db".my_table`
fn parse_table_reference(p: &mut Parser) {
    let m = p.start();

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier])
        && !at_end_of_column_list(p)
    {
        p.advance();

        if p.at(TokenKind::Dot) {
            p.advance();

            if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
                p.advance();
            } else {
                p.advance_with_error("Expected table name after dot");
            }
        }
    } else {
        p.advance_with_error("Expected table reference");
    }

    p.complete(m, SyntaxKind::TableIdentifier);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    fn check(input: &str, expected: Expect) {
        let result = parse(input);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        expected.assert_eq(&buf);
    }

    #[test]
    fn simple_select() {
        check("SELECT 1", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
        "#]]);
    }

    #[test]
    fn select_from() {
        check("SELECT a FROM t", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'a'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]]);
    }

    #[test]
    fn from_before_select() {
        check("FROM t SELECT a", expect![[r#"
            File
              SelectStatement
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'a'
        "#]]);
    }

    #[test]
    fn select_where_order_limit() {
        check("SELECT x FROM t WHERE x > 1 ORDER BY x LIMIT 10", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'x'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                WhereClause
                  'WHERE'
                  BinaryExpression
                    ColumnReference
                      'x'
                    '>'
                    NumberLiteral
                      '1'
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'x'
                LimitClause
                  'LIMIT'
                  NumberLiteral
                    '10'
        "#]]);
    }

    #[test]
    fn select_with_alias() {
        check("SELECT a AS b, c d FROM t", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'a'
                    ColumnAlias
                      'AS'
                      'b'
                    ','
                    ColumnReference
                      'c'
                    ColumnAlias
                      'd'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]]);
    }

    #[test]
    fn qualified_table_name() {
        check("SELECT 1 FROM db.table", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
                FromClause
                  'FROM'
                  TableIdentifier
                    'db'
                    '.'
                    'table'
        "#]]);
    }
}
