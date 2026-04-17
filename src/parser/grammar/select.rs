use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::{parse_expression, parse_window_spec};
use crate::parser::grammar::show::{at_explain_statement, parse_explain_statement};
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

    // WITH TOTALS can appear even without an explicit GROUP BY clause
    // (ClickHouse supports implicit grouping with WITH TOTALS).
    // If GROUP BY already consumed WITH TOTALS, this won't match.
    // Must be checked before skip_to_clause_keyword since WITH is not a clause keyword.
    if p.at_keyword(Keyword::With) && p.nth_text(1).eq_ignore_ascii_case("TOTALS") {
        let m = p.start();
        p.expect_keyword(Keyword::With);
        p.expect_keyword(Keyword::Totals);
        p.complete(m, SyntaxKind::WithTotalsClause);
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

    // WINDOW (named window definitions)
    if p.at_keyword(Keyword::Window) {
        parse_window_clause(p);
    }

    skip_to_clause_keyword(p);

    // QUALIFY — filters on window function results, like HAVING for GROUP BY
    if p.at_keyword(Keyword::Qualify) {
        let m = p.start();
        p.expect_keyword(Keyword::Qualify);
        parse_expression(p);
        p.complete(m, SyntaxKind::QualifyClause);
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

    // FORMAT
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

    // SETTINGS can also appear after FORMAT
    if p.at_keyword(Keyword::Settings) {
        parse_settings_clause(p);
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

/// True if the token at lookahead position `n` is a SELECT-statement clause
/// keyword that unambiguously starts a new clause. Used by expression
/// parsers to avoid consuming a clause keyword as if it were an identifier
/// (e.g. `SELECT t.|FROM x` — the adjacent `FROM` must not be swallowed
/// into the qualified column reference).
pub fn nth_is_clause_keyword(p: &mut Parser, n: usize) -> bool {
    if p.nth(n) != SyntaxKind::BareWord {
        return false;
    }
    let text = p.nth_text(n);
    for kw in SELECT_CLAUSE_KEYWORDS {
        if text.eq_ignore_ascii_case(kw.as_str()) {
            return true;
        }
    }
    // Unambiguous join-starter keywords. Ambiguous ones like LEFT/RIGHT/ANY
    // are excluded because they double as function names, so treating them
    // as clause-terminators mid-expression would over-recover.
    for kw in &[Keyword::Join, Keyword::Inner, Keyword::Cross, Keyword::Natural] {
        if text.eq_ignore_ascii_case(kw.as_str()) {
            return true;
        }
    }
    false
}

const SELECT_CLAUSE_KEYWORDS: &[Keyword] = &[
    Keyword::Select, Keyword::From, Keyword::Where, Keyword::Order,
    Keyword::Limit, Keyword::Group, Keyword::Having, Keyword::Prewhere,
    Keyword::Settings, Keyword::Format, Keyword::Union, Keyword::Except,
    Keyword::Intersect, Keyword::Window, Keyword::Sample, Keyword::Qualify,
];

/// True if the parser is positioned at a clause keyword that can appear
/// inside a SELECT statement. Used for error recovery.
///
/// `FORMAT` followed by `(` is treated as a function call (e.g. `format('{}', x)`),
/// not as the FORMAT clause keyword.
fn at_clause_keyword(p: &mut Parser) -> bool {
    // FORMAT is ambiguous: it can be the FORMAT clause or the format() function.
    // When followed by '(', it's a function call, not a clause.
    if p.at_keyword(Keyword::Format) && p.at_followed_by_paren() {
        return false;
    }
    // WITH TOTALS can appear as a standalone clause (even without GROUP BY)
    if p.at_keyword(Keyword::With) && p.nth_text(1).eq_ignore_ascii_case("TOTALS") {
        return true;
    }
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

    // These keywords can also be function names or identifiers; only treat them
    // as join keywords when NOT followed by '(' (function call), '.' (column ref),
    // ')' (inside parenthesized args like APPLY(any)), or ',' (inside arg lists).
    let at_ambiguous = !p.at_followed_by_paren()
        && p.nth(1) != SyntaxKind::Dot
        && p.nth(1) != SyntaxKind::ClosingRoundBracket
        && p.nth(1) != SyntaxKind::Comma
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
    // WITH RECURSIVE — optional RECURSIVE keyword for recursive CTEs
    p.eat_keyword(Keyword::Recursive);

    // The WITH clause can contain either:
    //   - CTE definitions: name AS (subquery), ...
    //   - Expression aliases: expr AS name, ...
    // We detect CTEs by checking: BareWord AS (
    parse_with_items(p);

    p.complete(m, SyntaxKind::WithClause);
}

/// Parse WITH clause items — a comma-separated list of CTEs or expression aliases.
fn parse_with_items(p: &mut Parser) {
    let m = p.start();

    let mut first = true;
    while !at_end_of_column_list(p) && !p.end_of_statement() {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        // Detect CTE pattern: identifier AS (
        if p.at_identifier()
            && p.nth(1) == SyntaxKind::BareWord
            && p.nth_text(1).eq_ignore_ascii_case("AS")
            && p.nth(2) == SyntaxKind::OpeningRoundBracket
        {
            // CTE: name AS (subquery)
            let item = p.start();
            p.advance(); // consume name
            p.expect_keyword(Keyword::As);
            p.expect(SyntaxKind::OpeningRoundBracket);
            if at_select_statement(p) {
                let subq = p.start();
                parse_select_statement(p);
                p.complete(subq, SyntaxKind::SubqueryExpression);
            } else {
                parse_expression(p);
            }
            p.expect(SyntaxKind::ClosingRoundBracket);
            p.complete(item, SyntaxKind::WithExpressionItem);
        } else {
            // Expression alias: expr AS name
            parse_expression(p);

            if p.at_keyword(Keyword::As)
                || (!at_end_of_column_list(p) && p.at(SyntaxKind::BareWord))
                || p.at(SyntaxKind::QuotedIdentifier)
            {
                let am = p.start();
                if p.at_keyword(Keyword::As) {
                    p.expect_keyword(Keyword::As);
                }
                if !at_end_of_column_list(p) {
                    p.advance();
                } else {
                    p.recover_with_error("Expected alias");
                }
                p.complete(am, SyntaxKind::ColumnAlias);
            }
        }
    }

    p.complete(m, SyntaxKind::ColumnList);
}

/// Parses: SELECT [DISTINCT [ON (col, ...)]] expr [, expr ...]
fn parse_select_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Select);

    // DISTINCT [ON (...)]
    if p.eat_keyword(Keyword::Distinct) && p.at_keyword(Keyword::On) {
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

/// Parses: FROM table_reference [, table_reference ...] [FINAL] [AS alias | alias]
/// Commas produce implicit cross joins (ClickHouse comma-join syntax).
fn parse_from_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::From);
    parse_table_reference(p);
    while p.at(SyntaxKind::Comma) && !p.eof() && !p.end_of_statement() {
        p.advance(); // consume comma
        parse_table_reference(p);
    }
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
                // SETTINGS clause inside table function arguments:
                // e.g. mysql('host', db, tbl, 'user', '', SETTINGS connect_timeout = 100)
                if p.at_keyword(Keyword::Settings)
                    && (p.nth(1) == SyntaxKind::BareWord || p.nth(1) == SyntaxKind::QuotedIdentifier)
                    && p.nth(2) == SyntaxKind::Equals
                {
                    common::parse_optional_settings_clause(p);
                    break;
                }
                if !first {
                    p.expect(SyntaxKind::Comma);
                    // Check for SETTINGS after comma
                    if p.at_keyword(Keyword::Settings)
                        && (p.nth(1) == SyntaxKind::BareWord || p.nth(1) == SyntaxKind::QuotedIdentifier)
                        && p.nth(2) == SyntaxKind::Equals
                    {
                        common::parse_optional_settings_clause(p);
                        break;
                    }
                }
                first = false;
                parse_expression(p);
            }
            p.expect(SyntaxKind::ClosingRoundBracket);
            p.complete(m, SyntaxKind::TableFunction);
        } else {
            p.complete(m, SyntaxKind::TableIdentifier);
        }

        // SAMPLE n [OFFSET m]
        if p.at_keyword(Keyword::Sample) {
            parse_sample_clause(p);
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

/// Parses: SAMPLE expr [OFFSET expr]
fn parse_sample_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Sample);
    parse_expression(p);
    if p.at_keyword(Keyword::Offset) {
        p.advance(); // OFFSET
        parse_expression(p);
    }
    p.complete(m, SyntaxKind::SampleClause);
}

/// Parses: (SELECT ...) or (EXPLAIN ...) as a table reference
fn parse_subquery_table_ref(p: &mut Parser) {
    let m = p.start();
    p.expect(SyntaxKind::OpeningRoundBracket);
    if at_select_statement(p) {
        parse_select_statement(p);
    } else if at_explain_statement(p) {
        parse_explain_statement(p);
    } else {
        p.recover_with_error("Expected subquery");
    }
    p.expect(SyntaxKind::ClosingRoundBracket);
    p.complete(m, SyntaxKind::SubqueryExpression);
}

/// Parses optional table alias: [AS] alias
/// Careful not to consume clause keywords as aliases.
///
/// When AS is present, it's unambiguous — any bareword is accepted as an alias
/// (including keywords like LEFT, RIGHT). Without AS, we reject keywords that
/// could start a JOIN or other clause.
fn parse_optional_table_alias(p: &mut Parser) {
    if p.at_keyword(Keyword::As) {
        let m = p.start();
        p.advance(); // consume AS
        if p.at(SyntaxKind::BareWord) || p.at(SyntaxKind::QuotedIdentifier) {
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

    // ARRAY JOIN is special — supports: [LEFT] ARRAY JOIN expr [AS alias], ...
    if p.at_keyword(Keyword::Array) {
        p.advance(); // ARRAY
        p.expect_keyword(Keyword::Join);
        // Comma-separated expression list with optional aliases
        let mut first = true;
        while !p.eof() && !p.end_of_statement() && !at_clause_keyword(p) && !at_join_keyword(p) {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;
            parse_expression(p);
            // Optional alias: AS alias or bare identifier alias
            if p.at_keyword(Keyword::As) {
                let am = p.start();
                p.advance(); // AS
                if p.at_identifier() && !at_clause_keyword(p) {
                    p.advance();
                } else {
                    p.recover_with_error("Expected alias after AS");
                }
                p.complete(am, SyntaxKind::ColumnAlias);
            } else if p.at(SyntaxKind::BareWord) && !at_clause_keyword(p) && !at_join_keyword(p) {
                let am = p.start();
                p.advance();
                p.complete(am, SyntaxKind::ColumnAlias);
            }
        }
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
///     or: GROUP BY GROUPING SETS ((...), (...), ...)
fn parse_group_by_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Group);
    p.expect_keyword(Keyword::By);

    // GROUP BY ALL — ClickHouse extension, like ORDER BY ALL.
    // ALL followed by `(` is the all() function, not the ALL keyword.
    if p.at_keyword(Keyword::All) && !p.at_followed_by_paren() {
        let cm = p.start();
        p.advance();
        p.complete(cm, SyntaxKind::ColumnReference);
    // GROUPING SETS ((...), (...), ...)
    } else if p.at_keyword(Keyword::Grouping) {
        parse_grouping_sets(p);
    } else {
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
            if p.at_keyword(Keyword::Totals)
                || p.at_keyword(Keyword::Rollup)
                || p.at_keyword(Keyword::Cube)
            {
                p.advance();
            } else {
                p.recover_with_error("Expected TOTALS, ROLLUP, or CUBE after WITH");
            }
        }
    }

    p.complete(m, SyntaxKind::GroupByClause);
}

/// Parses: GROUPING SETS ((expr, ...), (expr, ...), ...)
fn parse_grouping_sets(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Grouping);
    p.expect_keyword(Keyword::Sets);
    p.expect(SyntaxKind::OpeningRoundBracket);

    let mut first = true;
    while !p.eof() && !p.at(SyntaxKind::ClosingRoundBracket) {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        // Each grouping set is a parenthesized list of expressions (can be empty)
        let set_m = p.start();
        p.expect(SyntaxKind::OpeningRoundBracket);
        let mut first_expr = true;
        while !p.eof() && !p.at(SyntaxKind::ClosingRoundBracket) {
            if !first_expr {
                p.expect(SyntaxKind::Comma);
            }
            first_expr = false;
            parse_expression(p);
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
        p.complete(set_m, SyntaxKind::GroupingSet);
    }

    p.expect(SyntaxKind::ClosingRoundBracket);
    p.complete(m, SyntaxKind::GroupingSetsClause);
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
        || p.at_keyword(Keyword::Window)
        || p.at_keyword(Keyword::Qualify)
        || at_join_keyword(p)
}

// ========== WINDOW ==========

/// Parses: WINDOW name AS ( window_spec ) [, name AS ( window_spec ) ...]
fn parse_window_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Window);

    let mut first = true;
    while !p.eof() && !p.end_of_statement() && !at_order_by_terminator(p)
        && !p.at_keyword(Keyword::Window)
    {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        let wm = p.start();
        // window name
        if p.at_identifier() {
            p.advance();
        } else {
            p.recover_with_error("Expected window name");
        }
        p.expect_keyword(Keyword::As);
        parse_window_spec(p);
        p.complete(wm, SyntaxKind::WindowDefinition);
    }

    p.complete(m, SyntaxKind::WindowClause);
}

// ========== ORDER BY ==========

/// Parses: ORDER BY item, item, ...
/// Also handles ORDER BY ALL (ClickHouse extension).
fn parse_order_by_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Order);
    p.expect_keyword(Keyword::By);

    // ORDER BY ALL is a special ClickHouse syntax.
    // ALL is normally treated as a join keyword by at_order_by_terminator,
    // so we handle it explicitly before the loop.
    if p.at_keyword(Keyword::All) && !p.at_followed_by_paren() {
        let item_m = p.start();
        let cm = p.start();
        p.advance();
        p.complete(cm, SyntaxKind::ColumnReference);

        // ASC or DESC
        if p.at_keyword(Keyword::Asc) || p.at_keyword(Keyword::Desc) {
            p.advance();
        }

        p.complete(item_m, SyntaxKind::OrderByItem);
    } else {
        let mut first = true;
        while !p.eof() && !p.end_of_statement() && !at_order_by_terminator(p) {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;
            parse_order_by_item(p);
        }
    }

    p.complete(m, SyntaxKind::OrderByClause);
}

/// Parses: expr [ASC|DESC] [NULLS FIRST|LAST]
/// Also handles ORDER BY ALL (ClickHouse extension).
fn parse_order_by_item(p: &mut Parser) {
    let m = p.start();

    // ALL is a special ORDER BY target in ClickHouse.
    // It's normally treated as a join keyword by the expression parser,
    // so we handle it explicitly here.
    if p.at_keyword(Keyword::All) && !p.at_followed_by_paren() {
        let cm = p.start();
        p.advance();
        p.complete(cm, SyntaxKind::ColumnReference);
    } else {
        parse_expression(p);
    }

    // Optional alias: AS identifier
    // ClickHouse allows aliases in ORDER BY items: ORDER BY expr AS alias
    // Must check before ASC/DESC since AS is unambiguous here.
    if p.at_keyword(Keyword::As) {
        let am = p.start();
        p.advance(); // consume AS
        if p.at_identifier() && !at_order_by_terminator(p) {
            p.advance();
        } else {
            p.recover_with_error("Expected alias after AS");
        }
        p.complete(am, SyntaxKind::ColumnAlias);
    }

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

    // WITH FILL [FROM expr] [TO expr] [STEP expr] [INTERPOLATE (expr, ...)]
    if p.at_keyword(Keyword::With) && at_with_fill(p) {
        parse_with_fill_clause(p);
    }

    p.complete(m, SyntaxKind::OrderByItem);
}

/// Check if we're at WITH FILL (not WITH TOTALS/ROLLUP/CUBE)
fn at_with_fill(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::With)
        && p.nth(1) == SyntaxKind::BareWord
        && p.nth_text(1).eq_ignore_ascii_case("FILL")
}

/// Parses: WITH FILL [FROM expr] [TO expr] [STEP expr] [INTERPOLATE (expr, ...)]
fn parse_with_fill_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::With);
    p.expect_keyword(Keyword::Fill);

    // Optional FROM expr
    if p.at_keyword(Keyword::From) {
        p.advance();
        parse_expression(p);
    }

    // Optional TO expr
    if p.at_keyword(Keyword::To) {
        p.advance();
        parse_expression(p);
    }

    // Optional STEP expr
    if p.at_keyword(Keyword::Step) {
        p.advance();
        parse_expression(p);
    }

    // Optional INTERPOLATE (expr, ...)
    if p.at_keyword(Keyword::Interpolate) {
        p.advance();
        if p.at(SyntaxKind::OpeningRoundBracket) {
            p.advance(); // (
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

    p.complete(m, SyntaxKind::WithFillClause);
}

/// Keywords that terminate an ORDER BY item list.
fn at_order_by_terminator(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Limit)
        || p.at_keyword(Keyword::Settings)
        || (p.at_keyword(Keyword::Format) && !p.at_followed_by_paren())
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
        // WITH TIES
        if p.at_keyword(Keyword::With) && p.nth_keyword(1, Keyword::Ties) {
            p.advance(); // consume WITH
            p.expect_keyword(Keyword::Ties); // consume TIES (skips trivia)
        }
        p.complete(m, SyntaxKind::LimitClause);
    } else {
        // WITH TIES — consume both tokens if present
        if p.at_keyword(Keyword::With) && p.nth_keyword(1, Keyword::Ties) {
            p.advance(); // consume WITH
            p.expect_keyword(Keyword::Ties); // consume TIES (skips trivia)
        }
        // Plain LIMIT
        p.complete(m, SyntaxKind::LimitClause);
    }
}

/// Parse a plain LIMIT clause (used for the second LIMIT after LIMIT BY).
fn parse_limit_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Limit);
    parse_expression(p);

    if p.at_keyword(Keyword::Offset) || p.at(SyntaxKind::Comma) {
        p.advance();
        parse_expression(p);
    }

    // WITH TIES
    if p.at_keyword(Keyword::With) && p.nth_keyword(1, Keyword::Ties) {
        p.advance(); // consume WITH
        p.expect_keyword(Keyword::Ties); // consume TIES (skips trivia)
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
    while !p.eof()
        && !p.end_of_statement()
        && !p.at_keyword(Keyword::Select)
        && !p.at_keyword(Keyword::From)
        && !p.at_keyword(Keyword::Format)
        // Set operation keywords terminate SETTINGS
        && !p.at_keyword(Keyword::Union)
        && !p.at_keyword(Keyword::Except)
        && !p.at_keyword(Keyword::Intersect)
    {
        if !first {
            if !p.at(SyntaxKind::Comma) {
                break;
            }
            p.advance(); // comma
        }
        first = false;

        if !p.at_identifier() {
            break;
        }
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
    fn group_by_grouping_sets() {
        check("SELECT a, b, count() FROM t GROUP BY GROUPING SETS ((a, b), (a), ())", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'a'
                    ','
                    ColumnReference
                      'b'
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
                  GroupingSetsClause
                    'GROUPING'
                    'SETS'
                    '('
                    GroupingSet
                      '('
                      ColumnReference
                        'a'
                      ','
                      ColumnReference
                        'b'
                      ')'
                    ','
                    GroupingSet
                      '('
                      ColumnReference
                        'a'
                      ')'
                    ','
                    GroupingSet
                      '('
                      ')'
                    ')'
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

    #[test]
    fn window_over_clause() {
        check("SELECT sum(x) OVER (PARTITION BY y ORDER BY z)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    WindowExpression
                      FunctionCall
                        Identifier
                          'sum'
                        ExpressionList
                          '('
                          Expression
                            ColumnReference
                              'x'
                          ')'
                      'OVER'
                      WindowSpec
                        '('
                        'PARTITION'
                        'BY'
                        ColumnReference
                          'y'
                        'ORDER'
                        'BY'
                        ColumnReference
                          'z'
                        ')'
        "#]]);
    }

    #[test]
    fn window_over_name() {
        check("SELECT sum(x) OVER w FROM t", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    WindowExpression
                      FunctionCall
                        Identifier
                          'sum'
                        ExpressionList
                          '('
                          Expression
                            ColumnReference
                              'x'
                          ')'
                      'OVER'
                      'w'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]]);
    }

    #[test]
    fn window_with_frame() {
        check("SELECT sum(x) OVER (ORDER BY z ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    WindowExpression
                      FunctionCall
                        Identifier
                          'sum'
                        ExpressionList
                          '('
                          Expression
                            ColumnReference
                              'x'
                          ')'
                      'OVER'
                      WindowSpec
                        '('
                        'ORDER'
                        'BY'
                        ColumnReference
                          'z'
                        WindowFrame
                          'ROWS'
                          'BETWEEN'
                          'UNBOUNDED'
                          'PRECEDING'
                          'AND'
                          'CURRENT'
                          'ROW'
                        ')'
        "#]]);
    }

    #[test]
    fn window_clause() {
        check("SELECT sum(x) OVER w FROM t WINDOW w AS (ORDER BY z)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    WindowExpression
                      FunctionCall
                        Identifier
                          'sum'
                        ExpressionList
                          '('
                          Expression
                            ColumnReference
                              'x'
                          ')'
                      'OVER'
                      'w'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                WindowClause
                  'WINDOW'
                  WindowDefinition
                    'w'
                    'AS'
                    WindowSpec
                      '('
                      'ORDER'
                      'BY'
                      ColumnReference
                        'z'
                      ')'
        "#]]);
    }

    #[test]
    fn sample_clause() {
        check("SELECT * FROM t SAMPLE 0.1", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Asterisk
                      '*'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                  SampleClause
                    'SAMPLE'
                    NumberLiteral
                      '0.1'
        "#]]);
    }

    #[test]
    fn sample_with_offset() {
        check("SELECT * FROM t SAMPLE 0.1 OFFSET 0.5", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Asterisk
                      '*'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                  SampleClause
                    'SAMPLE'
                    NumberLiteral
                      '0.1'
                    'OFFSET'
                    NumberLiteral
                      '0.5'
        "#]]);
    }

    #[test]
    fn array_join_multiple() {
        check("SELECT * FROM t ARRAY JOIN arr1, arr2", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Asterisk
                      '*'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                ArrayJoinClause
                  'ARRAY'
                  'JOIN'
                  ColumnReference
                    'arr1'
                  ','
                  ColumnReference
                    'arr2'
        "#]]);
    }

    #[test]
    fn left_array_join_with_alias() {
        check("SELECT * FROM t LEFT ARRAY JOIN arr AS a", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Asterisk
                      '*'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                ArrayJoinClause
                  'LEFT'
                  'ARRAY'
                  'JOIN'
                  ColumnReference
                    'arr'
                  ColumnAlias
                    'AS'
                    'a'
        "#]]);
    }

    #[test]
    fn with_fill_clause() {
        check("SELECT date FROM t ORDER BY date WITH FILL FROM 0 TO 100 STEP 1", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'date'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
                OrderByClause
                  'ORDER'
                  'BY'
                  OrderByItem
                    ColumnReference
                      'date'
                    WithFillClause
                      'WITH'
                      'FILL'
                      'FROM'
                      NumberLiteral
                        '0'
                      'TO'
                      NumberLiteral
                        '100'
                      'STEP'
                      NumberLiteral
                        '1'
        "#]]);
    }

    #[test]
    fn limit_with_ties() {
        check("SELECT a FROM t LIMIT 1 WITH TIES", expect![[r#"
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
                    '1'
                  'WITH'
                  'TIES'
        "#]]);
    }

    #[test]
    fn except_set_operation_after_settings() {
        let result = parse("SELECT a FROM t SETTINGS min_hit_rate = 1.0 EXCEPT SELECT a FROM t2");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("UnionClause"), "tree should contain set operation: {}", buf);
        assert!(buf.contains("SettingsClause"), "tree should contain SETTINGS: {}", buf);
    }

    #[test]
    fn apply_bare_function_name() {
        check("SELECT a.* APPLY toString FROM t", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnTransformer
                      QualifiedAsterisk
                        ColumnReference
                          'a'
                        '.'
                        '*'
                      'APPLY'
                      ExpressionList
                        ColumnReference
                          'toString'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]]);
    }

    #[test]
    fn apply_chained() {
        let result = parse("SELECT a.* APPLY(toDate) APPLY(any) FROM t");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        // Two nested ColumnTransformer nodes
        assert!(buf.matches("ColumnTransformer").count() >= 2, "should have chained transformers: {}", buf);
    }

}
