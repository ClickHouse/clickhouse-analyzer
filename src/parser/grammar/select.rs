use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;

/// Parses a full SELECT statement:
///   [WITH ...] [FROM ... [FINAL] [AS alias]] [JOIN ...]
///   SELECT [DISTINCT [ON (...)]] ...
///   [FROM ... [FINAL] [AS alias]] [JOIN ...]
///   [PREWHERE expr]
///   [WHERE expr]
///   [GROUP BY expr, ... [WITH TOTALS|ROLLUP|CUBE]]
///   [HAVING expr]
///   [ORDER BY expr [ASC|DESC] [NULLS FIRST|LAST], ...]
///   [LIMIT n BY expr, ...]
///   [LIMIT n [OFFSET m] | LIMIT m, n]
///   [SETTINGS key=value, ...]
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
        parse_join_clauses(p);
        parsed_early_from = true;
    }

    parse_select_clause(p);

    if p.at_keyword(Keyword::From) {
        parse_from_clause(p);
        parse_join_clauses(p);

        if parsed_early_from {
            p.recover_with_error("Duplicate FROM clause");
        }
    }

    skip_to_clause_keyword(p);

    // PREWHERE
    if p.at_keyword(Keyword::Prewhere) {
        let m = p.start();
        p.expect_keyword(Keyword::Prewhere);
        parse_expression(p);
        p.complete(m, SyntaxKind::PrewhereClause);
    }

    skip_to_clause_keyword(p);

    // WHERE
    if p.at_keyword(Keyword::Where) {
        let m = p.start();
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
        p.complete(m, SyntaxKind::WhereClause);
    }

    skip_to_clause_keyword(p);

    // GROUP BY
    if p.at_keyword(Keyword::Group) {
        parse_group_by_clause(p);
    }

    skip_to_clause_keyword(p);

    // HAVING
    if p.at_keyword(Keyword::Having) {
        let m = p.start();
        p.expect_keyword(Keyword::Having);
        parse_expression(p);
        p.complete(m, SyntaxKind::HavingClause);
    }

    skip_to_clause_keyword(p);

    // ORDER BY
    if p.at_keyword(Keyword::Order) {
        parse_order_by_clause(p);
    }

    skip_to_clause_keyword(p);

    // LIMIT BY (must be checked before plain LIMIT)
    // We can't distinguish LIMIT ... BY from LIMIT ... until we see BY after the count.
    // So we parse LIMIT, then check for BY.
    if p.at_keyword(Keyword::Limit) {
        parse_limit_or_limit_by(p);
    }

    skip_to_clause_keyword(p);

    // After LIMIT BY, there can be a second plain LIMIT
    if p.at_keyword(Keyword::Limit) {
        parse_limit_clause(p);
    }

    skip_to_clause_keyword(p);

    // SETTINGS
    if p.at_keyword(Keyword::Settings) {
        parse_settings_clause(p);
    }

    skip_to_clause_keyword(p);

    // FORMAT (always last)
    if p.at_keyword(Keyword::Format) {
        let m = p.start();
        p.expect_keyword(Keyword::Format);
        if p.at_identifier() {
            p.advance();
        } else {
            p.recover_with_error("Expected format name after FORMAT");
        }
        p.complete(m, SyntaxKind::FormatClause);
    }

    let completed = p.complete(m, SyntaxKind::SelectStatement);

    // Set operations: UNION [ALL|DISTINCT], EXCEPT, INTERSECT
    if p.at_keyword(Keyword::Union)
        || p.at_keyword(Keyword::Except)
        || p.at_keyword(Keyword::Intersect)
    {
        let m = p.precede(completed);
        // Consume the set operation keyword
        p.advance();
        // Optional ALL or DISTINCT after UNION
        p.eat_keyword(Keyword::All);
        p.eat_keyword(Keyword::Distinct);
        // Parse the right-hand SELECT
        parse_select_statement(p);
        p.complete(m, SyntaxKind::UnionClause);
    }
}

/// True if the current position marks the end of a column list
/// (i.e. we've hit a clause keyword or statement boundary).
pub fn at_end_of_column_list(p: &mut Parser) -> bool {
    at_clause_keyword(p)
        || p.at_keyword(Keyword::Union)
        || p.at_keyword(Keyword::Except)
        || p.at_keyword(Keyword::Intersect)
}

const SELECT_CLAUSE_KEYWORDS: &[Keyword] = &[
    Keyword::Select, Keyword::From, Keyword::Where, Keyword::Order,
    Keyword::Limit, Keyword::Group, Keyword::Having, Keyword::Prewhere,
    Keyword::Settings, Keyword::Format, Keyword::Union, Keyword::Except,
    Keyword::Intersect,
];

/// True if the parser is positioned at a clause keyword that can appear
/// inside a SELECT statement. Used for error recovery.
fn at_clause_keyword(p: &mut Parser) -> bool {
    common::at_any_keyword(p, SELECT_CLAUSE_KEYWORDS) || at_join_keyword(p)
}

/// True if the parser is positioned at a keyword that starts a JOIN clause.
///
/// Keywords like ANY, ALL, etc. can also be used as function names in ClickHouse
/// (e.g. `any(col)`, `all(col)`). When followed by `(`, they are function calls,
/// not join keywords.
fn at_join_keyword(p: &mut Parser) -> bool {
    // These keywords unambiguously start a JOIN clause
    let at_unambiguous = p.at_keyword(Keyword::Join)
        || p.at_keyword(Keyword::Inner)
        || p.at_keyword(Keyword::Cross)
        || p.at_keyword(Keyword::Natural);

    // These keywords can also be function names; only treat them as join keywords
    // when NOT followed by '('
    let at_ambiguous = !p.at_followed_by_paren()
        && (p.at_keyword(Keyword::Left)
            || p.at_keyword(Keyword::Right)
            || p.at_keyword(Keyword::Full)
            || p.at_keyword(Keyword::Global)
            || p.at_keyword(Keyword::Any)
            || p.at_keyword(Keyword::All)
            || p.at_keyword(Keyword::Asof)
            || p.at_keyword(Keyword::Semi)
            || p.at_keyword(Keyword::Anti));

    at_unambiguous || at_ambiguous
        || p.at_keyword(Keyword::Array)
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

/// Parses: SELECT [DISTINCT [ON (col, ...)]] expr [, expr ...]
fn parse_select_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Select);

    // DISTINCT [ON (...)]
    if p.eat_keyword(Keyword::Distinct) {
        if p.at_keyword(Keyword::On) {
            p.advance(); // consume ON
            p.expect(SyntaxKind::OpeningRoundBracket);
            // parse comma-separated column list inside parens
            let mut first = true;
            while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() && !p.end_of_statement() {
                if !first {
                    p.expect(SyntaxKind::Comma);
                }
                first = false;
                parse_expression(p);
            }
            p.expect(SyntaxKind::ClosingRoundBracket);
        }
    }

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
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        parse_expression(p);

        if p.at_keyword(Keyword::As)
            || (!at_end_of_column_list(p) && p.at(SyntaxKind::BareWord))
            || p.at(SyntaxKind::QuotedIdentifier)
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

/// Parses: FROM table_reference [FINAL] [AS alias | alias]
fn parse_from_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::From);
    parse_table_reference(p);
    p.complete(m, SyntaxKind::FromClause);
}

/// Parses a table reference:
///   - identifier [. identifier] [FINAL] [[AS] alias]
///   - (SELECT ...) [[AS] alias]
///   - identifier(args) [[AS] alias]  (table function)
fn parse_table_reference(p: &mut Parser) {
    // Subquery: (SELECT ...)
    if p.at(SyntaxKind::OpeningRoundBracket) {
        parse_subquery_table_ref(p);
        parse_optional_table_alias(p);
        return;
    }

    if (p.at_identifier() || common::at_query_parameter(p))
        && !at_end_of_column_list(p)
    {
        let m = p.start();
        if common::at_query_parameter(p) {
            common::parse_query_parameter(p);
        } else {
            p.advance();
        }

        if p.at(SyntaxKind::Dot) {
            p.advance();
            if p.at_identifier() || common::at_query_parameter(p) {
                if common::at_query_parameter(p) {
                    common::parse_query_parameter(p);
                } else {
                    p.advance();
                }
            } else {
                p.advance_with_error("Expected table name after dot");
            }
        }

        // Check for table function: identifier(...)
        if p.at(SyntaxKind::OpeningRoundBracket) {
            // Table function call
            p.expect(SyntaxKind::OpeningRoundBracket);
            let mut first = true;
            while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() && !p.end_of_statement() {
                if !first {
                    p.expect(SyntaxKind::Comma);
                }
                first = false;
                parse_expression(p);
            }
            p.expect(SyntaxKind::ClosingRoundBracket);
            p.complete(m, SyntaxKind::TableFunction);
        } else {
            p.complete(m, SyntaxKind::TableIdentifier);
        }

        // FINAL
        p.eat_keyword(Keyword::Final);

        // Optional alias
        parse_optional_table_alias(p);
    } else {
        let m = p.start();
        p.advance_with_error("Expected table reference");
        p.complete(m, SyntaxKind::TableIdentifier);
    }
}

/// Parses: (SELECT ...) as a table reference
fn parse_subquery_table_ref(p: &mut Parser) {
    let m = p.start();
    p.expect(SyntaxKind::OpeningRoundBracket);
    if at_select_statement(p) {
        parse_select_statement(p);
    } else {
        p.recover_with_error("Expected subquery");
    }
    p.expect(SyntaxKind::ClosingRoundBracket);
    p.complete(m, SyntaxKind::SubqueryExpression);
}

/// Parses optional table alias: [AS] alias
/// Careful not to consume clause keywords as aliases.
fn parse_optional_table_alias(p: &mut Parser) {
    if p.at_keyword(Keyword::As) {
        let m = p.start();
        p.advance(); // consume AS
        if p.at_identifier() && !at_clause_keyword(p) {
            p.advance();
        } else {
            p.recover_with_error("Expected table alias");
        }
        p.complete(m, SyntaxKind::TableAlias);
    } else if p.at(SyntaxKind::BareWord) && !at_clause_keyword(p) && !at_join_keyword(p) && !p.at_keyword(Keyword::On) && !p.at_keyword(Keyword::Using) && !p.at_keyword(Keyword::Final) {
        let m = p.start();
        p.advance();
        p.complete(m, SyntaxKind::TableAlias);
    }
}

// ========== JOIN ==========

/// Parse zero or more JOIN clauses after a FROM clause.
fn parse_join_clauses(p: &mut Parser) {
    while at_join_keyword(p) && !p.eof() && !p.end_of_statement() {
        parse_join_clause(p);
    }
}

/// Parse a single JOIN clause:
///   [GLOBAL] [ANY|ALL|ASOF] [INNER|LEFT|RIGHT|FULL|CROSS] [OUTER|SEMI|ANTI] JOIN table_ref (ON expr | USING col_list)
fn parse_join_clause(p: &mut Parser) {
    let m = p.start();

    // Optional GLOBAL
    p.eat_keyword(Keyword::Global);

    // Optional strictness: ANY | ALL | ASOF
    if p.at_keyword(Keyword::Any) || p.at_keyword(Keyword::All) || p.at_keyword(Keyword::Asof) {
        p.advance();
    }

    // Optional join type: INNER | LEFT | RIGHT | FULL | CROSS | NATURAL
    if p.at_keyword(Keyword::Inner) || p.at_keyword(Keyword::Left) || p.at_keyword(Keyword::Right)
        || p.at_keyword(Keyword::Full) || p.at_keyword(Keyword::Cross) || p.at_keyword(Keyword::Natural)
    {
        p.advance();
    }

    // ARRAY JOIN is special
    if p.at_keyword(Keyword::Array) {
        p.advance();
        p.expect_keyword(Keyword::Join);
        // Array join has expression list, not table ref
        parse_expression(p);
        p.complete(m, SyntaxKind::ArrayJoinClause);
        return;
    }

    // Optional OUTER | SEMI | ANTI
    if p.at_keyword(Keyword::Outer) || p.at_keyword(Keyword::Semi) || p.at_keyword(Keyword::Anti) {
        p.advance();
    }

    p.expect_keyword(Keyword::Join);

    // Table reference
    parse_table_reference(p);

    // Join constraint: ON expr | USING col_list
    if p.at_keyword(Keyword::On) {
        p.advance();
        parse_expression(p);
    } else if p.at_keyword(Keyword::Using) {
        p.advance();
        // USING (col, col) or USING col
        if p.at(SyntaxKind::OpeningRoundBracket) {
            p.advance(); // consume (
            let mut first = true;
            while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() && !p.end_of_statement() {
                if !first {
                    p.expect(SyntaxKind::Comma);
                }
                first = false;
                parse_expression(p);
            }
            p.expect(SyntaxKind::ClosingRoundBracket);
        } else {
            // USING col (without parens)
            parse_expression(p);
        }
    }
    // CROSS JOIN has no constraint

    p.complete(m, SyntaxKind::JoinClause);
}

// ========== GROUP BY ==========

/// Parses: GROUP BY expr, ... [WITH TOTALS|ROLLUP|CUBE]
fn parse_group_by_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Group);
    p.expect_keyword(Keyword::By);

    let mut first = true;
    while !p.eof() && !p.end_of_statement() && !at_group_by_terminator(p) {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;
        parse_expression(p);
    }

    // WITH TOTALS | WITH ROLLUP | WITH CUBE
    if p.at_keyword(Keyword::With) {
        p.advance(); // consume WITH
        if p.at_keyword(Keyword::Totals) {
            p.advance();
        } else if p.at_keyword(Keyword::Rollup) {
            p.advance();
        } else if p.at_keyword(Keyword::Cube) {
            p.advance();
        } else {
            p.recover_with_error("Expected TOTALS, ROLLUP, or CUBE after WITH");
        }
    }

    p.complete(m, SyntaxKind::GroupByClause);
}

/// Keywords that terminate a GROUP BY expression list.
fn at_group_by_terminator(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Having)
        || p.at_keyword(Keyword::Order)
        || p.at_keyword(Keyword::Limit)
        || p.at_keyword(Keyword::Settings)
        || p.at_keyword(Keyword::Format)
        || p.at_keyword(Keyword::Select)
        || p.at_keyword(Keyword::From)
        || p.at_keyword(Keyword::Where)
        || p.at_keyword(Keyword::Prewhere)
        || p.at_keyword(Keyword::With)
        || at_join_keyword(p)
}

// ========== ORDER BY ==========

/// Parses: ORDER BY item, item, ...
fn parse_order_by_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Order);
    p.expect_keyword(Keyword::By);

    let mut first = true;
    while !p.eof() && !p.end_of_statement() && !at_order_by_terminator(p) {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;
        parse_order_by_item(p);
    }

    p.complete(m, SyntaxKind::OrderByClause);
}

/// Parses: expr [ASC|DESC] [NULLS FIRST|LAST]
fn parse_order_by_item(p: &mut Parser) {
    let m = p.start();
    parse_expression(p);

    // ASC or DESC
    if p.at_keyword(Keyword::Asc) || p.at_keyword(Keyword::Desc) {
        p.advance();
    }

    // NULLS FIRST | NULLS LAST
    if p.at_keyword(Keyword::Nulls) {
        p.advance();
        if p.at_keyword(Keyword::First) || p.at_keyword(Keyword::Last) {
            p.advance();
        } else {
            p.recover_with_error("Expected FIRST or LAST after NULLS");
        }
    }

    p.complete(m, SyntaxKind::OrderByItem);
}

/// Keywords that terminate an ORDER BY item list.
fn at_order_by_terminator(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Limit)
        || p.at_keyword(Keyword::Settings)
        || p.at_keyword(Keyword::Format)
        || p.at_keyword(Keyword::Select)
        || p.at_keyword(Keyword::From)
        || p.at_keyword(Keyword::Where)
        || p.at_keyword(Keyword::Prewhere)
        || p.at_keyword(Keyword::Having)
        || p.at_keyword(Keyword::Group)
        || at_join_keyword(p)
}

// ========== LIMIT / LIMIT BY ==========

/// Parses LIMIT, detecting whether it's LIMIT BY or plain LIMIT.
/// LIMIT n [OFFSET m] BY expr, ... => LimitByClause
/// LIMIT n [OFFSET m]              => LimitClause
/// LIMIT m, n                      => LimitClause
fn parse_limit_or_limit_by(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Limit);
    parse_expression(p);

    // Check for OFFSET before BY
    if p.at_keyword(Keyword::Offset) {
        p.advance();
        parse_expression(p);
    }

    if p.at_keyword(Keyword::By) {
        // LIMIT BY clause
        p.advance(); // consume BY
        let mut first = true;
        while !p.eof() && !p.end_of_statement() && !at_limit_by_terminator(p) {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;
            parse_expression(p);
        }
        p.complete(m, SyntaxKind::LimitByClause);
    } else if p.at(SyntaxKind::Comma) {
        // LIMIT m, n syntax (offset, count)
        p.advance(); // consume comma
        parse_expression(p);
        p.complete(m, SyntaxKind::LimitClause);
    } else {
        // Plain LIMIT
        p.complete(m, SyntaxKind::LimitClause);
    }
}

/// Parse a plain LIMIT clause (used for the second LIMIT after LIMIT BY).
fn parse_limit_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Limit);
    parse_expression(p);

    if p.at_keyword(Keyword::Offset) {
        p.advance();
        parse_expression(p);
    } else if p.at(SyntaxKind::Comma) {
        p.advance();
        parse_expression(p);
    }

    p.complete(m, SyntaxKind::LimitClause);
}

fn at_limit_by_terminator(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Limit)
        || p.at_keyword(Keyword::Settings)
        || p.at_keyword(Keyword::Format)
        || p.at_keyword(Keyword::Select)
        || p.at_keyword(Keyword::From)
        || p.at_keyword(Keyword::Where)
        || p.at_keyword(Keyword::Order)
        || at_join_keyword(p)
}

// ========== SETTINGS ==========

/// Parses: SETTINGS key = value, key = value, ...
fn parse_settings_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Settings);

    let mut first = true;
    while !p.eof() && !p.end_of_statement()
        && !p.at_keyword(Keyword::Select)
        && !p.at_keyword(Keyword::From)
        && !p.at_keyword(Keyword::Format)
    {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        common::parse_setting_item(p);
    }

    p.complete(m, SyntaxKind::SettingsClause);
}

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

    // ============ NEW TESTS ============

    #[test]
    fn select_distinct() {
        check("SELECT DISTINCT a, b FROM t", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  'DISTINCT'
                  ColumnList
                    ColumnReference
                      'a'
                    ','
                    ColumnReference
                      'b'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]]);
    }

    #[test]
    fn select_distinct_on() {
        check("SELECT DISTINCT ON (a) a, b FROM t", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  'DISTINCT'
                  'ON'
                  '('
                  ColumnReference
                    'a'
                  ')'
                  ColumnList
                    ColumnReference
                      'a'
                    ','
                    ColumnReference
                      'b'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]]);
    }

    #[test]
    fn group_by() {
        check("SELECT a FROM t GROUP BY a", expect![[r#"
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
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'a'
        "#]]);
    }

    #[test]
    fn group_by_with_totals() {
        check("SELECT a, count(*) FROM t GROUP BY a WITH TOTALS", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'a'
                    ','
                    FunctionCall
                      Identifier
                        'count'
                      ExpressionList
                        '('
                        Expression
                          Asterisk
                            '*'
                        ')'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'a'
                  'WITH'
                  'TOTALS'
        "#]]);
    }

    #[test]
    fn group_by_with_rollup() {
        check("SELECT a FROM t GROUP BY a WITH ROLLUP", expect![[r#"
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
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'a'
                  'WITH'
                  'ROLLUP'
        "#]]);
    }

    #[test]
    fn group_by_with_cube() {
        check("SELECT a FROM t GROUP BY a WITH CUBE", expect![[r#"
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
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'a'
                  'WITH'
                  'CUBE'
        "#]]);
    }

    #[test]
    fn group_by_having() {
        check("SELECT a FROM t GROUP BY a HAVING count(*) > 1", expect![[r#"
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
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'a'
                HavingClause
                  'HAVING'
                  BinaryExpression
                    FunctionCall
                      Identifier
                        'count'
                      ExpressionList
                        '('
                        Expression
                          Asterisk
                            '*'
                        ')'
                    '>'
                    NumberLiteral
                      '1'
        "#]]);
    }

    #[test]
    fn order_by_asc_desc() {
        check("SELECT a FROM t ORDER BY a ASC, b DESC", expect![[r#"
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
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'a'
                    'ASC'
                  ','
                  OrderByItem
                    ColumnReference
                      'b'
                    'DESC'
        "#]]);
    }

    #[test]
    fn order_by_nulls_first() {
        check("SELECT a FROM t ORDER BY a NULLS FIRST", expect![[r#"
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
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'a'
                    'NULLS'
                    'FIRST'
        "#]]);
    }

    #[test]
    fn order_by_desc_nulls_last() {
        check("SELECT a FROM t ORDER BY a DESC NULLS LAST", expect![[r#"
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
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'a'
                    'DESC'
                    'NULLS'
                    'LAST'
        "#]]);
    }

    #[test]
    fn limit_offset() {
        check("SELECT a FROM t LIMIT 10 OFFSET 5", expect![[r#"
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
                LimitClause
                  'LIMIT'
                  NumberLiteral
                    '10'
                  'OFFSET'
                  NumberLiteral
                    '5'
        "#]]);
    }

    #[test]
    fn limit_comma_syntax() {
        check("SELECT a FROM t LIMIT 5, 10", expect![[r#"
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
                LimitClause
                  'LIMIT'
                  NumberLiteral
                    '5'
                  ','
                  NumberLiteral
                    '10'
        "#]]);
    }

    #[test]
    fn prewhere_where() {
        check("SELECT a FROM t PREWHERE a > 0 WHERE b > 1", expect![[r#"
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
                PrewhereClause
                  'PREWHERE'
                  BinaryExpression
                    ColumnReference
                      'a'
                    '>'
                    NumberLiteral
                      '0'
                WhereClause
                  'WHERE'
                  BinaryExpression
                    ColumnReference
                      'b'
                    '>'
                    NumberLiteral
                      '1'
        "#]]);
    }

    #[test]
    fn settings_single() {
        check("SELECT a FROM t SETTINGS max_threads = 4", expect![[r#"
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
                SettingsClause
                  'SETTINGS'
                  SettingItem
                    'max_threads'
                    '='
                    NumberLiteral
                      '4'
        "#]]);
    }

    #[test]
    fn settings_multiple() {
        check("SELECT a FROM t SETTINGS max_threads = 4, timeout = 10", expect![[r#"
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
                SettingsClause
                  'SETTINGS'
                  SettingItem
                    'max_threads'
                    '='
                    NumberLiteral
                      '4'
                  ','
                  SettingItem
                    'timeout'
                    '='
                    NumberLiteral
                      '10'
        "#]]);
    }

    #[test]
    fn limit_by_then_limit() {
        check("SELECT a FROM t ORDER BY a LIMIT 3 BY a LIMIT 10", expect![[r#"
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
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'a'
                LimitByClause
                  'LIMIT'
                  NumberLiteral
                    '3'
                  'BY'
                  ColumnReference
                    'a'
                LimitClause
                  'LIMIT'
                  NumberLiteral
                    '10'
        "#]]);
    }

    #[test]
    fn inner_join_on() {
        check("SELECT a FROM t1 INNER JOIN t2 ON t1.id = t2.id", expect![[r#"
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
                    't1'
                JoinClause
                  'INNER'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't1'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't2'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn left_join_on() {
        check("SELECT a FROM t1 LEFT JOIN t2 ON t1.id = t2.id", expect![[r#"
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
                    't1'
                JoinClause
                  'LEFT'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't1'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't2'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn right_outer_join_using() {
        check("SELECT a FROM t1 RIGHT OUTER JOIN t2 USING (id)", expect![[r#"
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
                    't1'
                JoinClause
                  'RIGHT'
                  'OUTER'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'USING'
                  '('
                  ColumnReference
                    'id'
                  ')'
        "#]]);
    }

    #[test]
    fn cross_join() {
        check("SELECT a FROM t1 CROSS JOIN t2", expect![[r#"
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
                    't1'
                JoinClause
                  'CROSS'
                  'JOIN'
                  TableIdentifier
                    't2'
        "#]]);
    }

    #[test]
    fn global_left_join() {
        check("SELECT a FROM t1 GLOBAL LEFT JOIN t2 ON t1.id = t2.id", expect![[r#"
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
                    't1'
                JoinClause
                  'GLOBAL'
                  'LEFT'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't1'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't2'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn any_left_join_using() {
        check("SELECT a FROM t1 ANY LEFT JOIN t2 USING id", expect![[r#"
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
                    't1'
                JoinClause
                  'ANY'
                  'LEFT'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'USING'
                  ColumnReference
                    'id'
        "#]]);
    }

    #[test]
    fn multiple_joins() {
        check("SELECT a FROM t1 JOIN t2 ON t1.id = t2.id JOIN t3 ON t2.id = t3.id", expect![[r#"
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
                    't1'
                JoinClause
                  'JOIN'
                  TableIdentifier
                    't2'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't1'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't2'
                      '.'
                      'id'
                JoinClause
                  'JOIN'
                  TableIdentifier
                    't3'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't2'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't3'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn subquery_from() {
        check("SELECT a FROM (SELECT 1 AS x) AS sub", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'a'
                FromClause
                  'FROM'
                  SubqueryExpression
                    '('
                    SelectStatement
                      SelectClause
                        'SELECT'
                        ColumnList
                          NumberLiteral
                            '1'
                          ColumnAlias
                            'AS'
                            'x'
                    ')'
                  TableAlias
                    'AS'
                    'sub'
        "#]]);
    }

    #[test]
    fn table_function() {
        check("SELECT * FROM numbers(10)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Asterisk
                      '*'
                FromClause
                  'FROM'
                  TableFunction
                    'numbers'
                    '('
                    NumberLiteral
                      '10'
                    ')'
        "#]]);
    }

    #[test]
    fn select_from_final() {
        check("SELECT a FROM t FINAL", expect![[r#"
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
                  'FINAL'
        "#]]);
    }

    #[test]
    fn join_with_aliases() {
        check("SELECT a FROM t1 AS a JOIN t2 AS b ON a.id = b.id", expect![[r#"
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
                    't1'
                  TableAlias
                    'AS'
                    'a'
                JoinClause
                  'JOIN'
                  TableIdentifier
                    't2'
                  TableAlias
                    'AS'
                    'b'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      'a'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      'b'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn left_semi_join() {
        check("SELECT a FROM t1 LEFT SEMI JOIN t2 ON t1.id = t2.id", expect![[r#"
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
                    't1'
                JoinClause
                  'LEFT'
                  'SEMI'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't1'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't2'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn left_anti_join() {
        check("SELECT a FROM t1 LEFT ANTI JOIN t2 ON t1.id = t2.id", expect![[r#"
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
                    't1'
                JoinClause
                  'LEFT'
                  'ANTI'
                  'JOIN'
                  TableIdentifier
                    't2'
                  'ON'
                  BinaryExpression
                    ColumnReference
                      't1'
                      '.'
                      'id'
                    '='
                    ColumnReference
                      't2'
                      '.'
                      'id'
        "#]]);
    }

    #[test]
    fn format_clause() {
        check("SELECT 1 FORMAT JSON", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
                FormatClause
                  'FORMAT'
                  'JSON'
        "#]]);
    }

    #[test]
    fn format_clause_with_from() {
        check("SELECT col FROM t FORMAT JSONEachRow", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'col'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                FormatClause
                  'FORMAT'
                  'JSONEachRow'
        "#]]);
    }

    #[test]
    fn format_clause_after_settings() {
        check("SELECT 1 SETTINGS max_threads=4 FORMAT CSV", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
                SettingsClause
                  'SETTINGS'
                  SettingItem
                    'max_threads'
                    '='
                    NumberLiteral
                      '4'
                FormatClause
                  'FORMAT'
                  'CSV'
        "#]]);
    }

    #[test]
    fn format_clause_after_all_clauses() {
        check("SELECT a FROM t WHERE x > 1 ORDER BY a LIMIT 10 FORMAT TabSeparated", expect![[r#"
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
                      'a'
                LimitClause
                  'LIMIT'
                  NumberLiteral
                    '10'
                FormatClause
                  'FORMAT'
                  'TabSeparated'
        "#]]);
    }

    #[test]
    fn format_clause_after_order_by() {
        check("SELECT col FROM t ORDER BY col FORMAT JSON", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'col'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'col'
                FormatClause
                  'FORMAT'
                  'JSON'
        "#]]);
    }

    #[test]
    fn format_clause_after_group_by() {
        check("SELECT col FROM t GROUP BY col FORMAT JSON", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'col'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'col'
                FormatClause
                  'FORMAT'
                  'JSON'
        "#]]);
    }

    #[test]
    fn format_clause_after_limit_by() {
        check("SELECT col FROM t LIMIT 10 BY col FORMAT JSON", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'col'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                LimitByClause
                  'LIMIT'
                  NumberLiteral
                    '10'
                  'BY'
                  ColumnReference
                    'col'
                FormatClause
                  'FORMAT'
                  'JSON'
        "#]]);
    }

    #[test]
    fn format_clause_after_having() {
        check("SELECT col, count() FROM t GROUP BY col HAVING count() > 1 FORMAT CSV", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'col'
                    ','
                    FunctionCall
                      Identifier
                        'count'
                      ExpressionList
                        '('
                        ')'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                GroupByClause
                  'GROUP'
                  'BY'
                  ColumnReference
                    'col'
                HavingClause
                  'HAVING'
                  BinaryExpression
                    FunctionCall
                      Identifier
                        'count'
                      ExpressionList
                        '('
                        ')'
                    '>'
                    NumberLiteral
                      '1'
                FormatClause
                  'FORMAT'
                  'CSV'
        "#]]);
    }
}
