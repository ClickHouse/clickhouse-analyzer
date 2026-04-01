use clickhouse_analyzer::{format, parse, FormatConfig};
use expect_test::{expect, Expect};

fn check_format(input: &str, expected: Expect) {
    let result = parse(input);
    let formatted = format(&result.tree, &FormatConfig::default());
    expected.assert_eq(&formatted);
}

fn check_idempotent(input: &str) {
    let result = parse(input);
    let formatted = format(&result.tree, &FormatConfig::default());
    let result2 = parse(&formatted);
    let formatted2 = format(&result2.tree, &FormatConfig::default());
    assert_eq!(formatted, formatted2, "Formatting is not idempotent");
}

// ---------------------------------------------------------------------------
// Basic SELECT
// ---------------------------------------------------------------------------

#[test]
fn simple_select() {
    check_format(
        "select 1",
        expect![[r#"
            SELECT
                1
        "#]],
    );
}

#[test]
fn select_columns() {
    check_format(
        "select a, b, c from t",
        expect![[r#"
            SELECT
                a,
                b,
                c
            FROM t
        "#]],
    );
}

#[test]
fn select_single_column() {
    check_format(
        "select a from t",
        expect![[r#"
            SELECT
                a
            FROM t
        "#]],
    );
}

#[test]
fn select_with_alias() {
    check_format(
        "select sum(a) as total, b from t",
        expect![[r#"
            SELECT
                sum(a) AS total,
                b
            FROM t
        "#]],
    );
}

#[test]
fn select_star() {
    check_format(
        "select * from t",
        expect![[r#"
            SELECT
                *
            FROM t
        "#]],
    );
}

// ---------------------------------------------------------------------------
// WHERE
// ---------------------------------------------------------------------------

#[test]
fn select_where() {
    check_format(
        "select  a  from  t  where  x > 1",
        expect![[r#"
            SELECT
                a
            FROM t
            WHERE x > 1
        "#]],
    );
}

#[test]
fn where_compound_condition() {
    check_format(
        "select a from t where x>1 and y<10 or z=5",
        expect![[r#"
            SELECT
                a
            FROM t
            WHERE x > 1 AND y < 10 OR z = 5
        "#]],
    );
}

// ---------------------------------------------------------------------------
// GROUP BY / ORDER BY
// ---------------------------------------------------------------------------

#[test]
fn group_by_single() {
    check_format(
        "select a from t group by a",
        expect![[r#"
            SELECT
                a
            FROM t
            GROUP BY a
        "#]],
    );
}

#[test]
fn group_by_multiple() {
    check_format(
        "select a,b from t group by a,b",
        expect![[r#"
            SELECT
                a,
                b
            FROM t
            GROUP BY
                a,
                b
        "#]],
    );
}

#[test]
fn order_by_single() {
    check_format(
        "select a from t order by a desc",
        expect![[r#"
            SELECT
                a
            FROM t
            ORDER BY a DESC
        "#]],
    );
}

#[test]
fn order_by_multiple() {
    check_format(
        "select a from t order by a desc, b asc",
        expect![[r#"
            SELECT
                a
            FROM t
            ORDER BY
                a DESC,
                b ASC
        "#]],
    );
}

// ---------------------------------------------------------------------------
// LIMIT
// ---------------------------------------------------------------------------

#[test]
fn limit() {
    check_format(
        "select a from t limit 10",
        expect![[r#"
            SELECT
                a
            FROM t
            LIMIT 10
        "#]],
    );
}

#[test]
fn limit_offset() {
    check_format(
        "select a from t limit 10 offset 5",
        expect![[r#"
            SELECT
                a
            FROM t
            LIMIT 10 OFFSET 5
        "#]],
    );
}

// ---------------------------------------------------------------------------
// JOIN
// ---------------------------------------------------------------------------

#[test]
fn inner_join() {
    check_format(
        "select a from t1 inner join t2 on t1.id=t2.id",
        expect![[r#"
            SELECT
                a
            FROM t1
            INNER JOIN t2 ON t1.id = t2.id
        "#]],
    );
}

#[test]
fn left_join() {
    check_format(
        "select a from t1 left join t2 on t1.id=t2.id",
        expect![[r#"
            SELECT
                a
            FROM t1
            LEFT JOIN t2 ON t1.id = t2.id
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[test]
fn binary_expression_spacing() {
    check_format(
        "select 1+2, a*b, x-y from t",
        expect![[r#"
            SELECT
                1 + 2,
                a * b,
                x - y
            FROM t
        "#]],
    );
}

#[test]
fn function_call() {
    check_format(
        "select count(*), sum(a), max(b) from t",
        expect![[r#"
            SELECT
                count(*),
                sum(a),
                max(b)
            FROM t
        "#]],
    );
}

#[test]
fn nested_function_call() {
    check_format(
        "select toDate(now()) from t",
        expect![[r#"
            SELECT
                toDate(now())
            FROM t
        "#]],
    );
}

#[test]
fn case_expression() {
    check_format(
        "select case when x>1 then 'a' when x>2 then 'b' else 'c' end from t",
        expect![[r#"
            SELECT
                CASE
                    WHEN x > 1 THEN 'a'
                    WHEN x > 2 THEN 'b'
                    ELSE 'c'
                END
            FROM t
        "#]],
    );
}

#[test]
fn subquery() {
    check_format(
        "select * from (select a, b from t where x > 1)",
        expect![[r#"
            SELECT
                *
            FROM (
                SELECT
                    a,
                    b
                FROM t
                WHERE x > 1
            )
        "#]],
    );
}

#[test]
fn in_expression() {
    check_format(
        "select a from t where x in (1,2,3)",
        expect![[r#"
            SELECT
                a
            FROM t
            WHERE x IN (1, 2, 3)
        "#]],
    );
}

#[test]
fn cast_double_colon() {
    check_format(
        "select a::Int32 from t",
        expect![[r#"
            SELECT
                a::Int32
            FROM t
        "#]],
    );
}

#[test]
fn array_literal() {
    check_format(
        "select [1,2,3]",
        expect![[r#"
            SELECT
                [1, 2, 3]
        "#]],
    );
}

#[test]
fn tuple_literal() {
    check_format(
        "select (1,2,3)",
        expect![[r#"
            SELECT
                (1, 2, 3)
        "#]],
    );
}

#[test]
fn unary_minus() {
    check_format(
        "select -1, -a from t",
        expect![[r#"
            SELECT
                -1,
                -a
            FROM t
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Multiple statements
// ---------------------------------------------------------------------------

#[test]
fn multiple_statements() {
    check_format(
        "select a from t; select b from u",
        expect![[r#"
            SELECT
                a
            FROM t;

            SELECT
                b
            FROM u
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Keyword case normalization
// ---------------------------------------------------------------------------

#[test]
fn keyword_uppercase() {
    check_format(
        "Select a From t Where x > 1 Group By a Order By a Desc Limit 10",
        expect![[r#"
            SELECT
                a
            FROM t
            WHERE x > 1
            GROUP BY a
            ORDER BY a DESC
            LIMIT 10
        "#]],
    );
}

#[test]
fn lowercase_keywords() {
    let result = parse("SELECT a FROM t WHERE x > 1");
    let config = FormatConfig {
        indent_width: 4,
        uppercase_keywords: false,
    };
    let formatted = format(&result.tree, &config);
    assert!(formatted.contains("select"));
    assert!(formatted.contains("from"));
    assert!(formatted.contains("where"));
    assert!(!formatted.contains("SELECT"));
}

// ---------------------------------------------------------------------------
// Indentation config
// ---------------------------------------------------------------------------

#[test]
fn custom_indent_width() {
    let result = parse("select a, b from t");
    let config = FormatConfig {
        indent_width: 2,
        uppercase_keywords: true,
    };
    let formatted = format(&result.tree, &config);
    assert!(formatted.contains("  a,"), "Expected 2-space indent, got:\n{formatted}");
}

// ---------------------------------------------------------------------------
// Messy input cleanup
// ---------------------------------------------------------------------------

#[test]
fn messy_whitespace() {
    check_format(
        "select    a  ,  b   from    t    where   x  >  1",
        expect![[r#"
            SELECT
                a,
                b
            FROM t
            WHERE x > 1
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Idempotency
// ---------------------------------------------------------------------------

#[test]
fn idempotent_simple() {
    check_idempotent("select a, b from t where x > 1 order by a limit 10");
}

#[test]
fn idempotent_complex() {
    check_idempotent(
        "select a, b, sum(c) as total from t1 inner join t2 on t1.id = t2.id where x > 1 group by a, b order by total desc limit 100",
    );
}

#[test]
fn idempotent_subquery() {
    check_idempotent("select * from (select a from t where x > 1)");
}

#[test]
fn idempotent_case() {
    check_idempotent("select case when x > 1 then 'a' else 'b' end from t");
}

// ---------------------------------------------------------------------------
// Error resilience
// ---------------------------------------------------------------------------

#[test]
fn formatter_handles_parse_errors() {
    // Invalid SQL should not panic, and should produce some output
    let result = parse("select from where");
    let formatted = format(&result.tree, &FormatConfig::default());
    assert!(!formatted.is_empty(), "Formatter should produce output even for invalid SQL");
}

#[test]
fn formatter_handles_garbage() {
    let result = parse("!@#$%");
    let formatted = format(&result.tree, &FormatConfig::default());
    assert!(!formatted.is_empty(), "Formatter should handle garbage input");
}

#[test]
fn formatter_handles_empty_input() {
    let result = parse("");
    let formatted = format(&result.tree, &FormatConfig::default());
    assert_eq!(formatted, "");
}

// ---------------------------------------------------------------------------
// Full integration
// ---------------------------------------------------------------------------

#[test]
fn full_query() {
    check_format(
        "select a,b,c from t1 inner join t2 on t1.id=t2.id left join t3 on t2.x=t3.x where a>1 and b<10 group by a,b order by c desc, a asc limit 10",
        expect![[r#"
            SELECT
                a,
                b,
                c
            FROM t1
            INNER JOIN t2 ON t1.id = t2.id
            LEFT JOIN t3 ON t2.x = t3.x
            WHERE a > 1 AND b < 10
            GROUP BY
                a,
                b
            ORDER BY
                c DESC,
                a ASC
            LIMIT 10
        "#]],
    );
}
