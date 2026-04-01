use crate::lexer::token::TokenKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Parses a data type with optional parameters:
///   `Int32`, `String`, `Array(UInt64)`, `Tuple(Int32, String)`, `Nullable(Float64)`
///   `DateTime64(9)`, `Decimal(18, 4)`, `FixedString(100)`, `Enum8('a' = 1, 'b' = 2)`
pub fn parse_column_type(p: &mut Parser) {
    let m = p.start();

    if p.at(TokenKind::BareWord) {
        p.advance();
    } else {
        p.advance_with_error("Expected type name");
    }

    if p.at(TokenKind::OpeningRoundBracket) {
        let m = p.start();
        p.expect(TokenKind::OpeningRoundBracket);

        if !p.at(TokenKind::ClosingRoundBracket) {
            parse_type_parameter(p);
            while p.at(TokenKind::Comma) && !p.eof() {
                p.expect(TokenKind::Comma);
                parse_type_parameter(p);
            }
        }

        p.expect(TokenKind::ClosingRoundBracket);
        p.complete(m, SyntaxKind::DataTypeParameters);
    }

    p.complete(m, SyntaxKind::DataType);
}

/// Parse a single type parameter: either a nested type (BareWord) or a literal/expression.
fn parse_type_parameter(p: &mut Parser) {
    if p.at(TokenKind::BareWord) {
        // Nested type like Array(String), Nullable(UInt64)
        parse_column_type(p);
    } else if !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
        // Numeric/string parameter like DateTime64(9), FixedString(100), Enum8('a' = 1)
        parse_expression(p);
    }
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
    fn simple_type_cast() {
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
    fn nested_type_cast() {
        check("SELECT x::Array(Tuple(Int32, String))", expect![[r#"
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
                        'Array'
                        DataTypeParameters
                          '('
                          DataType
                            'Tuple'
                            DataTypeParameters
                              '('
                              DataType
                                'Int32'
                              ','
                              DataType
                                'String'
                              ')'
                          ')'
        "#]]);
    }
}
