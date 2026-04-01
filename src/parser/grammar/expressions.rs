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
    let Some(mut lhs) = expr_delimited(p) else {
        p.advance_with_error("Expected expression");
        return;
    };

    // Parametric function calls: `func(params)(args)`
    while p.at(TokenKind::OpeningRoundBracket) {
        let m = p.precede(lhs);
        arg_list(p);
        lhs = p.complete(m, SyntaxKind::FunctionCall);
    }

    // Binary operators via Pratt precedence climbing
    loop {
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

/// Parses a "delimited" (atomic) expression and any postfix operators.
///
/// Atoms: identifiers, literals, `*`, parenthesized expressions, arrays, subqueries, INTERVAL.
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
            let m = p.start();
            if p.at_keyword(Keyword::Interval) {
                parse_interval_expression(p);
                p.complete(m, SyntaxKind::IntervalExpression)
            } else if at_select_statement(p) {
                parse_select_statement(p);
                p.complete(m, SyntaxKind::SubqueryExpression)
            } else if !at_end_of_column_list(p) {
                p.advance();
                while p.at(TokenKind::Dot) && !p.eof() {
                    p.advance();
                    expr_delimited(p);
                }
                p.complete(m, SyntaxKind::ColumnReference)
            } else {
                p.recover_with_error("Expected column identifier");
                p.complete(m, SyntaxKind::ColumnReference)
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
}
