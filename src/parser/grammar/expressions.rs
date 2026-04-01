use crate::lexer::token::TokenKind;
use crate::parser::grammar::select::{at_end_of_column_list, at_select_statement, parse_select_statement};
use crate::parser::grammar::types::parse_column_type;
use crate::parser::interval_unit::IntervalUnit;
use crate::parser::keyword::Keyword;
use crate::parser::marker::CompletedMarker;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Binding power levels for binary operators (higher = tighter).
///
/// Modeled after ClickHouse's operator precedence:
/// https://clickhouse.com/docs/en/sql-reference/operators
#[derive(Clone, Copy)]
enum BinOp {
    // Logical (lowest)
    Or,
    And,
    // Comparison
    Equals,
    NotEquals,
    Less,
    Greater,
    LessOrEquals,
    GreaterOrEquals,
    // Arithmetic
    Plus,
    Minus,
    Asterisk,
    Slash,
}

impl BinOp {
    /// Try to read a binary operator from the parser's current position.
    /// Handles both token operators (+, -, etc.) and keyword operators (AND, OR).
    fn from_parser(p: &mut Parser) -> Option<BinOp> {
        if p.at_keyword(Keyword::And) {
            return Some(BinOp::And);
        }
        if p.at_keyword(Keyword::Or) {
            return Some(BinOp::Or);
        }
        match p.nth(0) {
            TokenKind::Plus => Some(BinOp::Plus),
            TokenKind::Minus => Some(BinOp::Minus),
            TokenKind::Asterisk => Some(BinOp::Asterisk),
            TokenKind::Slash => Some(BinOp::Slash),
            TokenKind::Equals => Some(BinOp::Equals),
            TokenKind::NotEquals => Some(BinOp::NotEquals),
            TokenKind::Less => Some(BinOp::Less),
            TokenKind::Greater => Some(BinOp::Greater),
            TokenKind::LessOrEquals => Some(BinOp::LessOrEquals),
            TokenKind::GreaterOrEquals => Some(BinOp::GreaterOrEquals),
            _ => None,
        }
    }

    fn binding_power(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Equals | BinOp::NotEquals => 3,
            BinOp::Less | BinOp::Greater | BinOp::LessOrEquals | BinOp::GreaterOrEquals => 4,
            BinOp::Plus | BinOp::Minus => 5,
            BinOp::Asterisk | BinOp::Slash => 6,
        }
    }
}

pub fn parse_expression(p: &mut Parser) {
    parse_expression_rec(p, 0);
}

fn parse_expression_rec(p: &mut Parser, min_bp: u8) {
    // Handle prefix NOT: binding power 3 (between AND=2 and comparisons=3..4)
    // NOT binds tighter than AND/OR but at the same level as comparisons
    if p.at_keyword(Keyword::Not) {
        let m = p.start();
        p.advance(); // consume NOT
        parse_expression_rec(p, 3);
        let lhs = p.complete(m, SyntaxKind::UnaryExpression);
        // Continue with binary operators after the NOT expression
        parse_expression_postfix(p, lhs, min_bp);
        return;
    }

    // Handle prefix unary minus: highest precedence (7)
    if p.at(TokenKind::Minus) {
        let m = p.start();
        p.advance(); // consume -
        parse_expression_rec(p, 7);
        let lhs = p.complete(m, SyntaxKind::UnaryExpression);
        parse_expression_postfix(p, lhs, min_bp);
        return;
    }

    let Some(mut lhs) = expr_delimited(p) else {
        p.advance_with_error("Expected expression");
        return;
    };

    // Postfix operators that chain: function calls `f(x)` and array access `a[k]`
    loop {
        if p.at(TokenKind::OpeningRoundBracket) {
            let m = p.precede(lhs);
            arg_list(p);
            lhs = p.complete(m, SyntaxKind::FunctionCall);
        } else if p.at(TokenKind::OpeningSquareBracket) {
            let m = p.precede(lhs);
            p.advance(); // consume [
            parse_expression(p);
            p.expect(TokenKind::ClosingSquareBracket);
            lhs = p.complete(m, SyntaxKind::ArrayAccessExpression);
        } else {
            break;
        }
    }

    parse_expression_postfix(p, lhs, min_bp);
}

/// Handles postfix and infix operators after an initial LHS expression.
fn parse_expression_postfix(p: &mut Parser, mut lhs: CompletedMarker, min_bp: u8) {
    loop {
        // IS [NOT] NULL — postfix, binding power 4
        if p.at_keyword(Keyword::Is) && 4 > min_bp {
            let m = p.precede(lhs);
            p.advance(); // consume IS
            p.eat_keyword(Keyword::Not); // optional NOT
            p.expect_keyword(Keyword::Null);
            lhs = p.complete(m, SyntaxKind::IsNullExpression);
            continue;
        }

        // [NOT] BETWEEN expr AND expr — infix, binding power 4
        if p.at_keyword(Keyword::Between) && 4 > min_bp {
            let m = p.precede(lhs);
            p.advance(); // consume BETWEEN
            parse_expression_rec(p, 5); // parse low bound (above +/- level)
            p.expect_keyword(Keyword::And);
            parse_expression_rec(p, 5); // parse high bound
            lhs = p.complete(m, SyntaxKind::BetweenExpression);
            continue;
        }

        // NOT BETWEEN, NOT IN, NOT LIKE — prefix NOT modifying a postfix operator
        if p.at_keyword(Keyword::Not) && 4 > min_bp {
            // Peek ahead to see if it's NOT BETWEEN, NOT IN, or NOT LIKE
            // We need to check the next non-trivia token after NOT
            if is_not_followed_by_postfix_op(p) {
                let m = p.precede(lhs);
                p.advance(); // consume NOT

                if p.at_keyword(Keyword::Between) {
                    p.advance(); // consume BETWEEN
                    parse_expression_rec(p, 5);
                    p.expect_keyword(Keyword::And);
                    parse_expression_rec(p, 5);
                    lhs = p.complete(m, SyntaxKind::BetweenExpression);
                } else if p.at_keyword(Keyword::In) {
                    p.advance(); // consume IN
                    parse_in_rhs(p);
                    lhs = p.complete(m, SyntaxKind::InExpression);
                } else if p.at_keyword(Keyword::Like) {
                    p.advance(); // consume LIKE
                    parse_expression_rec(p, 4);
                    lhs = p.complete(m, SyntaxKind::LikeExpression);
                }
                continue;
            }
        }

        // [GLOBAL] [NOT] IN (...)
        if p.at_keyword(Keyword::Global) && 4 > min_bp {
            let m = p.precede(lhs);
            p.advance(); // consume GLOBAL
            p.eat_keyword(Keyword::Not); // optional NOT
            p.expect_keyword(Keyword::In);
            parse_in_rhs(p);
            lhs = p.complete(m, SyntaxKind::InExpression);
            continue;
        }

        if p.at_keyword(Keyword::In) && 4 > min_bp {
            let m = p.precede(lhs);
            p.advance(); // consume IN
            parse_in_rhs(p);
            lhs = p.complete(m, SyntaxKind::InExpression);
            continue;
        }

        // LIKE — infix, binding power 4
        if p.at_keyword(Keyword::Like) && 4 > min_bp {
            let m = p.precede(lhs);
            p.advance(); // consume LIKE
            parse_expression_rec(p, 4);
            lhs = p.complete(m, SyntaxKind::LikeExpression);
            continue;
        }

        // Binary operators via Pratt precedence climbing
        let Some(op) = BinOp::from_parser(p) else {
            break;
        };
        if op.binding_power() <= min_bp {
            break;
        }

        let m = p.precede(lhs);
        p.advance();
        parse_expression_rec(p, op.binding_power());
        lhs = p.complete(m, SyntaxKind::BinaryExpression);
    }
}

/// Check if we're at NOT followed by BETWEEN, IN, or LIKE
fn is_not_followed_by_postfix_op(p: &mut Parser) -> bool {
    // We know p is at NOT. We need to peek past NOT to see what follows.
    // The parser skips trivia on nth(), so nth(1) looks at the next non-trivia token after current.
    if p.nth(0) != TokenKind::BareWord {
        return false;
    }
    // Look at the token after NOT (skipping whitespace between them)
    // We need to check raw tokens to peek ahead
    let text = p.nth_text(0);
    if !text.eq_ignore_ascii_case("NOT") {
        return false;
    }
    // Now check what's after NOT — we need to look at position 1
    // nth(1) should give us the next non-trivia token
    let next = p.nth(1);
    if next != TokenKind::BareWord {
        return false;
    }
    let next_text = p.nth_text(1);
    next_text.eq_ignore_ascii_case("BETWEEN")
        || next_text.eq_ignore_ascii_case("IN")
        || next_text.eq_ignore_ascii_case("LIKE")
}

/// Parse the right-hand side of an IN expression: (expr, ...) or (subquery)
fn parse_in_rhs(p: &mut Parser) {
    if p.at(TokenKind::OpeningRoundBracket) {
        p.expect(TokenKind::OpeningRoundBracket);
        if !p.at(TokenKind::ClosingRoundBracket) {
            // Check if it's a subquery
            if at_select_statement(p) {
                let m = p.start();
                parse_select_statement(p);
                p.complete(m, SyntaxKind::SubqueryExpression);
            } else {
                parse_expression(p);
                while p.at(TokenKind::Comma) && !p.eof() {
                    p.advance();
                    parse_expression(p);
                }
            }
        }
        p.expect(TokenKind::ClosingRoundBracket);
    } else {
        // Could be a table name or other expression
        parse_expression_rec(p, 4);
    }
}

/// Parses a "delimited" (atomic) expression and any postfix operators.
///
/// Atoms: identifiers, literals, `*`, parenthesized expressions, arrays, maps,
/// subqueries, INTERVAL, CASE, NULL, TRUE, FALSE.
/// Postfix: `::` cast operator.
fn expr_delimited(p: &mut Parser) -> Option<CompletedMarker> {
    let result = match p.nth(0) {
        TokenKind::Asterisk => {
            let m = p.start();
            p.advance();
            p.complete(m, SyntaxKind::Asterisk)
        }
        TokenKind::StringLiteral => {
            let m = p.start();
            p.advance();
            p.complete(m, SyntaxKind::StringLiteral)
        }
        TokenKind::Number => {
            let m = p.start();
            p.advance();
            p.complete(m, SyntaxKind::NumberLiteral)
        }
        TokenKind::BareWord | TokenKind::QuotedIdentifier => {
            // NULL literal
            if p.at_keyword(Keyword::Null) {
                let m = p.start();
                p.advance();
                p.complete(m, SyntaxKind::NullLiteral)
            }
            // Boolean literals
            else if p.at_keyword(Keyword::True) || p.at_keyword(Keyword::False) {
                let m = p.start();
                p.advance();
                p.complete(m, SyntaxKind::BooleanLiteral)
            }
            // CASE expression
            else if p.at_keyword(Keyword::Case) {
                parse_case_expression(p)
            }
            // INTERVAL expression
            else if p.at_keyword(Keyword::Interval) {
                let m = p.start();
                parse_interval_expression(p);
                p.complete(m, SyntaxKind::IntervalExpression)
            }
            // Subquery starting with SELECT/FROM/WITH
            else if at_select_statement(p) {
                let m = p.start();
                parse_select_statement(p);
                p.complete(m, SyntaxKind::SubqueryExpression)
            }
            // Regular identifier / column reference
            else if !at_end_of_column_list(p) {
                let m = p.start();
                p.advance();
                while p.at(TokenKind::Dot) && !p.eof() {
                    p.advance();
                    expr_delimited(p);
                }
                p.complete(m, SyntaxKind::ColumnReference)
            } else {
                return None;
            }
        }
        // Parenthesized expression or tuple: (expr) or (expr, expr, ...)
        TokenKind::OpeningRoundBracket => {
            let m = p.start();
            p.expect(TokenKind::OpeningRoundBracket);
            let mut count = 0;
            if !p.at(TokenKind::ClosingRoundBracket) {
                parse_expression(p);
                count += 1;
                while p.at(TokenKind::Comma) && !p.eof() {
                    p.advance();
                    parse_expression(p);
                    count += 1;
                }
            }

            p.expect(TokenKind::ClosingRoundBracket);
            if count > 1 {
                p.complete(m, SyntaxKind::TupleExpression)
            } else {
                p.complete(m, SyntaxKind::Expression)
            }
        }
        // Array literal: [expr, expr, ...] or []
        TokenKind::OpeningSquareBracket => {
            let m = p.start();
            p.expect(TokenKind::OpeningSquareBracket);

            if !p.at(TokenKind::ClosingSquareBracket) {
                parse_expression(p);

                while p.at(TokenKind::Comma) && !p.eof() {
                    p.advance();
                    parse_expression(p);
                }
            }

            p.expect(TokenKind::ClosingSquareBracket);
            p.complete(m, SyntaxKind::ArrayExpression)
        }
        // Map literal: {key: value, ...} or {}
        TokenKind::OpeningCurlyBrace => {
            let m = p.start();
            p.expect(TokenKind::OpeningCurlyBrace);

            if !p.at(TokenKind::ClosingCurlyBrace) {
                parse_expression(p);
                p.expect(TokenKind::Colon);
                parse_expression(p);

                while p.at(TokenKind::Comma) && !p.eof() {
                    p.advance();
                    parse_expression(p);
                    p.expect(TokenKind::Colon);
                    parse_expression(p);
                }
            }

            p.expect(TokenKind::ClosingCurlyBrace);
            p.complete(m, SyntaxKind::MapExpression)
        }
        _ => return None,
    };

    // Postfix cast: expr::Type
    if p.at(TokenKind::DoubleColon) {
        let m = p.precede(result);
        p.expect(TokenKind::DoubleColon);
        parse_column_type(p);
        return Some(p.complete(m, SyntaxKind::CastExpression));
    }

    Some(result)
}

/// Parses: CASE [expr] WHEN expr THEN expr [WHEN ... THEN ...] [ELSE expr] END
fn parse_case_expression(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect_keyword(Keyword::Case);

    // Optional operand for simple CASE (CASE x WHEN 1 THEN ...)
    if !p.at_keyword(Keyword::When) && !p.at_keyword(Keyword::End) && !p.eof() {
        parse_expression(p);
    }

    // WHEN ... THEN ... clauses
    while p.at_keyword(Keyword::When) && !p.eof() {
        let w = p.start();
        p.advance(); // consume WHEN
        parse_expression(p);
        p.expect_keyword(Keyword::Then);
        parse_expression(p);
        p.complete(w, SyntaxKind::WhenClause);
    }

    // Optional ELSE
    if p.eat_keyword(Keyword::Else) {
        parse_expression(p);
    }

    p.expect_keyword(Keyword::End);
    p.complete(m, SyntaxKind::CaseExpression)
}

fn at_interval_unit(p: &mut Parser) -> bool {
    p.nth(0) == TokenKind::BareWord && IntervalUnit::from_str(p.nth_text(0)).is_some()
}

/// Parses: INTERVAL expr UNIT
/// e.g. `INTERVAL 5 MINUTE`, `INTERVAL (1 + 2) DAY`
fn parse_interval_expression(p: &mut Parser) {
    p.expect_keyword(Keyword::Interval);
    parse_expression(p);
    if at_interval_unit(p) {
        p.advance();
    } else {
        p.recover_with_error("Expected interval unit (e.g. SECOND, MINUTE, HOUR, DAY, WEEK, MONTH, QUARTER, YEAR)");
    }
}

/// Parses a parenthesized argument list: (arg, arg, ...)
fn arg_list(p: &mut Parser) {
    let m = p.start();

    let mut first = true;
    p.expect(TokenKind::OpeningRoundBracket);
    while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
        if !first {
            p.expect(TokenKind::Comma);
        }
        arg(p);
        first = false;
    }
    p.expect(TokenKind::ClosingRoundBracket);

    p.complete(m, SyntaxKind::ExpressionList);
}

/// Parses a single function argument, detecting lambda expressions (x -> expr).
fn arg(p: &mut Parser) {
    let m = p.start();
    parse_expression(p);

    if p.at(TokenKind::Arrow) {
        p.advance();
        parse_expression(p);
        p.complete(m, SyntaxKind::LambdaExpression);
        return;
    }

    p.complete(m, SyntaxKind::Expression);
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
    fn binary_precedence() {
        check("SELECT 1 + 2 * 3", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    BinaryExpression
                      NumberLiteral
                        '1'
                      '+'
                      BinaryExpression
                        NumberLiteral
                          '2'
                        '*'
                        NumberLiteral
                          '3'
        "#]]);
    }

    #[test]
    fn function_call() {
        check("SELECT now()", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    FunctionCall
                      ColumnReference
                        'now'
                      ExpressionList
                        '('
                        ')'
        "#]]);
    }

    #[test]
    fn parametric_function() {
        check("SELECT quantile(0.9)(x)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    FunctionCall
                      FunctionCall
                        ColumnReference
                          'quantile'
                        ExpressionList
                          '('
                          Expression
                            NumberLiteral
                              '0.9'
                          ')'
                      ExpressionList
                        '('
                        Expression
                          ColumnReference
                            'x'
                        ')'
        "#]]);
    }

    #[test]
    fn cast_expression() {
        check("SELECT x::Int32", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    CastExpression
                      ColumnReference
                        'x'
                      '::'
                      DataType
                        'Int32'
        "#]]);
    }

    #[test]
    fn interval_expression() {
        check("SELECT INTERVAL 5 MINUTE", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    IntervalExpression
                      'INTERVAL'
                      NumberLiteral
                        '5'
                      'MINUTE'
        "#]]);
    }

    #[test]
    fn array_literal() {
        check("SELECT [1, 2, 3]", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayExpression
                      '['
                      NumberLiteral
                        '1'
                      ','
                      NumberLiteral
                        '2'
                      ','
                      NumberLiteral
                        '3'
                      ']'
        "#]]);
    }

    #[test]
    fn lambda_in_function() {
        check("SELECT arrayMap(x -> x + 1, arr)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    FunctionCall
                      ColumnReference
                        'arrayMap'
                      ExpressionList
                        '('
                        LambdaExpression
                          ColumnReference
                            'x'
                          '->'
                          BinaryExpression
                            ColumnReference
                              'x'
                            '+'
                            NumberLiteral
                              '1'
                        ','
                        Expression
                          ColumnReference
                            'arr'
                        ')'
        "#]]);
    }

    #[test]
    fn logical_operators() {
        check("SELECT 1 FROM t WHERE a > 1 AND b < 2 OR c = 3", expect![[r#"
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
                    't'
                WhereClause
                  'WHERE'
                  BinaryExpression
                    BinaryExpression
                      BinaryExpression
                        ColumnReference
                          'a'
                        '>'
                        NumberLiteral
                          '1'
                      'AND'
                      BinaryExpression
                        ColumnReference
                          'b'
                        '<'
                        NumberLiteral
                          '2'
                    'OR'
                    BinaryExpression
                      ColumnReference
                        'c'
                      '='
                      NumberLiteral
                        '3'
        "#]]);
    }

    #[test]
    fn nested_dot_access() {
        check("SELECT json.nested.path", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ColumnReference
                      'json'
                      '.'
                      ColumnReference
                        'nested'
                        '.'
                        ColumnReference
                          'path'
        "#]]);
    }

    // === New expression type tests ===

    #[test]
    fn unary_not() {
        check("SELECT NOT true", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    UnaryExpression
                      'NOT'
                      BooleanLiteral
                        'true'
        "#]]);
    }

    #[test]
    fn unary_not_not() {
        check("SELECT NOT NOT false", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    UnaryExpression
                      'NOT'
                      UnaryExpression
                        'NOT'
                        BooleanLiteral
                          'false'
        "#]]);
    }

    #[test]
    fn unary_minus() {
        check("SELECT -1", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    UnaryExpression
                      '-'
                      NumberLiteral
                        '1'
        "#]]);
    }

    #[test]
    fn unary_minus_paren() {
        check("SELECT -(1 + 2)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    UnaryExpression
                      '-'
                      Expression
                        '('
                        BinaryExpression
                          NumberLiteral
                            '1'
                          '+'
                          NumberLiteral
                            '2'
                        ')'
        "#]]);
    }

    #[test]
    fn between_expression() {
        check("SELECT x BETWEEN 1 AND 10", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    BetweenExpression
                      ColumnReference
                        'x'
                      'BETWEEN'
                      NumberLiteral
                        '1'
                      'AND'
                      NumberLiteral
                        '10'
        "#]]);
    }

    #[test]
    fn not_between_expression() {
        check("SELECT x NOT BETWEEN 1 AND 10", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    BetweenExpression
                      ColumnReference
                        'x'
                      'NOT'
                      'BETWEEN'
                      NumberLiteral
                        '1'
                      'AND'
                      NumberLiteral
                        '10'
        "#]]);
    }

    #[test]
    fn in_expression() {
        check("SELECT x IN (1, 2, 3)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    InExpression
                      ColumnReference
                        'x'
                      'IN'
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
        "#]]);
    }

    #[test]
    fn not_in_expression() {
        check("SELECT x NOT IN (1, 2, 3)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    InExpression
                      ColumnReference
                        'x'
                      'NOT'
                      'IN'
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
        "#]]);
    }

    #[test]
    fn global_in_expression() {
        check("SELECT x GLOBAL IN (SELECT 1)", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    InExpression
                      ColumnReference
                        'x'
                      'GLOBAL'
                      'IN'
                      '('
                      SubqueryExpression
                        SelectStatement
                          SelectClause
                            'SELECT'
                            ColumnList
                              NumberLiteral
                                '1'
                      ')'
        "#]]);
    }

    #[test]
    fn like_expression() {
        check("SELECT x LIKE '%test%'", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    LikeExpression
                      ColumnReference
                        'x'
                      'LIKE'
                      StringLiteral
                        ''%test%''
        "#]]);
    }

    #[test]
    fn not_like_expression() {
        check("SELECT x NOT LIKE '%test%'", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    LikeExpression
                      ColumnReference
                        'x'
                      'NOT'
                      'LIKE'
                      StringLiteral
                        ''%test%''
        "#]]);
    }

    #[test]
    fn is_null_expression() {
        check("SELECT x IS NULL", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    IsNullExpression
                      ColumnReference
                        'x'
                      'IS'
                      'NULL'
        "#]]);
    }

    #[test]
    fn is_not_null_expression() {
        check("SELECT x IS NOT NULL", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    IsNullExpression
                      ColumnReference
                        'x'
                      'IS'
                      'NOT'
                      'NULL'
        "#]]);
    }

    #[test]
    fn case_when_else() {
        check("SELECT CASE WHEN x > 1 THEN 'a' ELSE 'b' END", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    CaseExpression
                      'CASE'
                      WhenClause
                        'WHEN'
                        BinaryExpression
                          ColumnReference
                            'x'
                          '>'
                          NumberLiteral
                            '1'
                        'THEN'
                        StringLiteral
                          ''a''
                      'ELSE'
                      StringLiteral
                        ''b''
                      'END'
        "#]]);
    }

    #[test]
    fn case_simple() {
        check("SELECT CASE x WHEN 1 THEN 'one' WHEN 2 THEN 'two' END", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    CaseExpression
                      'CASE'
                      ColumnReference
                        'x'
                      WhenClause
                        'WHEN'
                        NumberLiteral
                          '1'
                        'THEN'
                        StringLiteral
                          ''one''
                      WhenClause
                        'WHEN'
                        NumberLiteral
                          '2'
                        'THEN'
                        StringLiteral
                          ''two''
                      'END'
        "#]]);
    }

    #[test]
    fn null_literal() {
        check("SELECT NULL", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NullLiteral
                      'NULL'
        "#]]);
    }

    #[test]
    fn boolean_literals() {
        check("SELECT TRUE, FALSE", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    BooleanLiteral
                      'TRUE'
                    ','
                    BooleanLiteral
                      'FALSE'
        "#]]);
    }

    #[test]
    fn map_literal() {
        check("SELECT {'key': 'value', 'k2': 'v2'}", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    MapExpression
                      '{'
                      StringLiteral
                        ''key''
                      ':'
                      StringLiteral
                        ''value''
                      ','
                      StringLiteral
                        ''k2''
                      ':'
                      StringLiteral
                        ''v2''
                      '}'
        "#]]);
    }

    #[test]
    fn empty_map_literal() {
        check("SELECT {}", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    MapExpression
                      '{'
                      '}'
        "#]]);
    }

    // === Array access (subscript) tests ===

    #[test]
    fn array_access_string_key() {
        // Map-style access: SpanAttributes['test.key']
        check("SELECT SpanAttributes['test.key']", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayAccessExpression
                      ColumnReference
                        'SpanAttributes'
                      '['
                      StringLiteral
                        ''test.key''
                      ']'
        "#]]);
    }

    #[test]
    fn array_access_numeric_index() {
        check("SELECT arr[1]", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayAccessExpression
                      ColumnReference
                        'arr'
                      '['
                      NumberLiteral
                        '1'
                      ']'
        "#]]);
    }

    #[test]
    fn array_access_chained() {
        // Nested access: matrix[0][1]
        check("SELECT matrix[0][1]", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayAccessExpression
                      ArrayAccessExpression
                        ColumnReference
                          'matrix'
                        '['
                        NumberLiteral
                          '0'
                        ']'
                      '['
                      NumberLiteral
                        '1'
                      ']'
        "#]]);
    }

    #[test]
    fn array_access_on_function_result() {
        // Access on function call: splitByChar(',', x)[1]
        check("SELECT splitByChar(',', x)[1]", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayAccessExpression
                      FunctionCall
                        ColumnReference
                          'splitByChar'
                        ExpressionList
                          '('
                          Expression
                            StringLiteral
                              '',''
                          ','
                          Expression
                            ColumnReference
                              'x'
                          ')'
                      '['
                      NumberLiteral
                        '1'
                      ']'
        "#]]);
    }

    #[test]
    fn array_access_with_expression_key() {
        // Dynamic key: arr[i + 1]
        check("SELECT arr[i + 1]", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayAccessExpression
                      ColumnReference
                        'arr'
                      '['
                      BinaryExpression
                        ColumnReference
                          'i'
                        '+'
                        NumberLiteral
                          '1'
                      ']'
        "#]]);
    }

    #[test]
    fn array_access_in_where_clause() {
        check("SELECT 1 FROM t WHERE attrs['status'] = 'ok'", expect![[r#"
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
                    't'
                WhereClause
                  'WHERE'
                  BinaryExpression
                    ArrayAccessExpression
                      ColumnReference
                        'attrs'
                      '['
                      StringLiteral
                        ''status''
                      ']'
                    '='
                    StringLiteral
                      ''ok''
        "#]]);
    }

    #[test]
    fn array_access_with_dot_and_subscript() {
        // Dotted column + subscript: otel.SpanAttributes['key']
        check("SELECT otel.SpanAttributes['key']", expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayAccessExpression
                      ColumnReference
                        'otel'
                        '.'
                        ColumnReference
                          'SpanAttributes'
                      '['
                      StringLiteral
                        ''key''
                      ']'
        "#]]);
    }
}
