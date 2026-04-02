use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;

/// Parses a data type with optional parameters:
///   `Int32`, `String`, `Array(UInt64)`, `Tuple(Int32, String)`, `Nullable(Float64)`
///   `DateTime64(9)`, `Decimal(18, 4)`, `FixedString(100)`, `Enum8('a' = 1, 'b' = 2)`
pub fn parse_column_type(p: &mut Parser) {
    let m = p.start();

    if p.at(SyntaxKind::BareWord) {
        p.advance();
    } else {
        p.advance_with_error("Expected type name");
    }

    if p.at(SyntaxKind::OpeningRoundBracket) {
        let m = p.start();
        p.expect(SyntaxKind::OpeningRoundBracket);

        if !p.at(SyntaxKind::ClosingRoundBracket) {
            parse_type_parameter(p);
            while p.at(SyntaxKind::Comma) && !p.eof() {
                p.expect(SyntaxKind::Comma);
                parse_type_parameter(p);
            }
        }

        p.expect(SyntaxKind::ClosingRoundBracket);
        p.complete(m, SyntaxKind::DataTypeParameters);
    }

    p.complete(m, SyntaxKind::DataType);
}

/// Parse a single type parameter: either a nested type (BareWord) or a literal/expression.
///
/// Handles named tuple fields: `Tuple(a String, b String)` where a BareWord
/// field name precedes the type.  Disambiguated by checking whether two
/// consecutive BareWords appear — if so the first is a field name.
///
/// Also handles key=value parameters: `Dynamic(max_dynamic_paths=254)` where
/// a BareWord is followed by `=` and an expression.
fn parse_type_parameter(p: &mut Parser) {
    if p.at(SyntaxKind::BareWord) {
        if p.at_keyword(Keyword::Skip) {
            // SKIP path — JSON type parameter to skip a path.
            // Consume SKIP keyword and the path identifier(s).
            p.advance(); // consume SKIP
            // The path can be a dot-separated identifier or a REGEXP pattern
            if p.at(SyntaxKind::BareWord) {
                p.advance(); // consume path name
                // Handle dot-separated paths like SKIP a.b.c
                while p.at(SyntaxKind::Dot) && p.nth(1) == SyntaxKind::BareWord {
                    p.advance(); // consume .
                    p.advance(); // consume identifier
                }
            } else if p.at(SyntaxKind::StringToken) {
                // SKIP REGEXP 'pattern' — already consumed SKIP, string follows
                p.advance();
            }
        } else if p.nth(1) == SyntaxKind::Equals {
            // Key=value parameter: `max_dynamic_paths=254`
            parse_expression(p);
        } else if p.nth(1) == SyntaxKind::BareWord {
            // Named field: `name Type` — consume the name, then parse the type.
            p.advance(); // field name
            parse_column_type(p);
        } else {
            // Nested type like Array(String), Nullable(UInt64)
            parse_column_type(p);
        }
    } else if !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
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
        result.tree.print(&mut buf, 0, &result.source);
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

    #[test]
    fn numeric_type_parameter() {
        check("SELECT x::DateTime64(9)", expect![[r#"
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
                        'DateTime64'
                        DataTypeParameters
                          '('
                          NumberLiteral
                            '9'
                          ')'
        "#]]);
    }

    #[test]
    fn decimal_type_parameters() {
        check("SELECT x::Decimal(18, 4)", expect![[r#"
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
                        'Decimal'
                        DataTypeParameters
                          '('
                          NumberLiteral
                            '18'
                          ','
                          NumberLiteral
                            '4'
                          ')'
        "#]]);
    }

    #[test]
    fn fixed_string_type() {
        check("SELECT x::FixedString(100)", expect![[r#"
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
                        'FixedString'
                        DataTypeParameters
                          '('
                          NumberLiteral
                            '100'
                          ')'
        "#]]);
    }

    #[test]
    fn enum_type() {
        check("SELECT x::Enum8('a' = 1, 'b' = 2)", expect![[r#"
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
                        'Enum8'
                        DataTypeParameters
                          '('
                          BinaryExpression
                            StringLiteral
                              ''a''
                            '='
                            NumberLiteral
                              '1'
                          ','
                          BinaryExpression
                            StringLiteral
                              ''b''
                            '='
                            NumberLiteral
                              '2'
                          ')'
        "#]]);
    }

    #[test]
    fn nested_type_with_numeric_param() {
        check("SELECT x::Array(DateTime64(9))", expect![[r#"
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
                            'DateTime64'
                            DataTypeParameters
                              '('
                              NumberLiteral
                                '9'
                              ')'
                          ')'
        "#]]);
    }

    #[test]
    fn map_type_with_lowcardinality() {
        check("SELECT x::Map(LowCardinality(String), String)", expect![[r#"
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
                        'Map'
                        DataTypeParameters
                          '('
                          DataType
                            'LowCardinality'
                            DataTypeParameters
                              '('
                              DataType
                                'String'
                              ')'
                          ','
                          DataType
                            'String'
                          ')'
        "#]]);
    }
}
