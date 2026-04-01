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

// ---------------------------------------------------------------------------
// INSERT
// ---------------------------------------------------------------------------

#[test]
fn insert_values() {
    check_format(
        "insert into t values (1, 2, 3)",
        expect![[r#"
            INSERT INTO t
            VALUES (1, 2, 3)
        "#]],
    );
}

#[test]
fn insert_with_columns() {
    check_format(
        "insert into t (a, b) values (1, 2)",
        expect![[r#"
            INSERT INTO t (a, b)
            VALUES (1, 2)
        "#]],
    );
}

#[test]
fn insert_select() {
    check_format(
        "insert into t select 1, 2 from u",
        expect![[r#"
            INSERT INTO t
            SELECT
                1,
                2
            FROM u
        "#]],
    );
}

// ---------------------------------------------------------------------------
// DROP / USE / SET
// ---------------------------------------------------------------------------

#[test]
fn drop_table() {
    check_format(
        "drop table if exists db.t",
        expect![[r#"
            DROP TABLE IF EXISTS db.t
        "#]],
    );
}

#[test]
fn use_database() {
    check_format(
        "use mydb",
        expect![[r#"
            USE mydb
        "#]],
    );
}

#[test]
fn set_statement() {
    check_format(
        "set max_threads = 4",
        expect![[r#"
            SET
                max_threads = 4
        "#]],
    );
}

// ---------------------------------------------------------------------------
// EXPLAIN / DESCRIBE / SHOW
// ---------------------------------------------------------------------------

#[test]
fn explain_ast() {
    check_format(
        "explain ast select 1",
        expect![[r#"
            EXPLAIN AST
                SELECT
                    1
        "#]],
    );
}

#[test]
fn describe_table() {
    check_format(
        "describe table t",
        expect![[r#"
            DESCRIBE TABLE t
        "#]],
    );
}

#[test]
fn show_tables() {
    check_format(
        "show tables from mydb like '%t%'",
        expect![[r#"
            SHOW TABLES FROM mydb LIKE '%t%'
        "#]],
    );
}

// ---------------------------------------------------------------------------
// UNION
// ---------------------------------------------------------------------------

#[test]
fn union_all() {
    // Use UPDATE_EXPECT=1 to auto-update if formatting changes
    let result = parse("select 1 union all select 2");
    let formatted = format(&result.tree, &FormatConfig::default());
    // Just verify it contains the right structure
    assert!(formatted.contains("SELECT"), "Should contain SELECT");
    assert!(formatted.contains("UNION"), "Should contain UNION");
    assert!(formatted.contains("ALL"), "Should contain ALL");
}

// ---------------------------------------------------------------------------
// DELETE
// ---------------------------------------------------------------------------

#[test]
fn delete_from() {
    check_format(
        "delete from t where x > 1",
        expect![[r#"
            DELETE FROM t WHERE x > 1
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Idempotency for new statements
// ---------------------------------------------------------------------------

#[test]
fn idempotent_insert() {
    check_idempotent("INSERT INTO t VALUES (1, 2, 3)");
}

#[test]
fn idempotent_drop() {
    check_idempotent("DROP TABLE IF EXISTS db.t");
}

#[test]
fn idempotent_delete() {
    check_idempotent("DELETE FROM t WHERE x > 1");
}

#[test]
fn idempotent_union() {
    check_idempotent("SELECT 1 UNION ALL SELECT 2");
}

// ---------------------------------------------------------------------------
// FORMAT clause
// ---------------------------------------------------------------------------

#[test]
fn format_clause_simple() {
    check_format(
        "select 1 format JSON",
        expect![[r#"
            SELECT
                1
            FORMAT JSON
        "#]],
    );
}

#[test]
fn format_clause_after_settings() {
    check_format(
        "select 1 settings max_threads=4 format JSONEachRow",
        expect![[r#"
            SELECT
                1
            SETTINGS max_threads = 4
            FORMAT JSONEachRow
        "#]],
    );
}

#[test]
fn format_clause_with_full_query() {
    check_format(
        "select a from t where x > 1 order by a limit 10 format CSV",
        expect![[r#"
            SELECT
                a
            FROM t
            WHERE x > 1
            ORDER BY a
            LIMIT 10
            FORMAT CSV
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Data type casing preservation
// ---------------------------------------------------------------------------

#[test]
fn data_type_preserves_casing() {
    check_format(
        "SELECT x::Array(String)",
        expect![[r#"
            SELECT
                x::Array(String)
        "#]],
    );
}

#[test]
fn data_type_lowcardinality() {
    check_format(
        "SELECT x::LowCardinality(String)",
        expect![[r#"
            SELECT
                x::LowCardinality(String)
        "#]],
    );
}

#[test]
fn data_type_numeric_param() {
    check_format(
        "SELECT x::DateTime64(9)",
        expect![[r#"
            SELECT
                x::DateTime64(9)
        "#]],
    );
}

#[test]
fn data_type_map_nested() {
    check_format(
        "SELECT x::Map(LowCardinality(String), String)",
        expect![[r#"
            SELECT
                x::Map(LowCardinality(String), String)
        "#]],
    );
}

// ---------------------------------------------------------------------------
// CODEC / Engine / Index tight parens and casing
// ---------------------------------------------------------------------------

#[test]
fn codec_tight_parens_and_casing() {
    check_format(
        "CREATE TABLE t (`ts` DateTime64(9) CODEC(Delta(8), ZSTD(1))) ENGINE = MergeTree() ORDER BY ts",
        expect![[r#"
            CREATE TABLE t
            (
                `ts` DateTime64(9) CODEC(Delta(8), ZSTD(1))
            )
            ENGINE = MergeTree()
            ORDER BY ts
        "#]],
    );
}

#[test]
fn engine_preserves_casing() {
    check_format(
        "CREATE TABLE t (id UInt64) ENGINE = SharedMergeTree('/path', '{replica}') ORDER BY id",
        expect![[r#"
            CREATE TABLE t
            (
                id UInt64
            )
            ENGINE = SharedMergeTree('/path', '{replica}')
            ORDER BY id
        "#]],
    );
}

#[test]
fn index_type_tight_parens() {
    check_format(
        "CREATE TABLE t (id UInt64, INDEX idx id TYPE bloom_filter(0.01) GRANULARITY 1) ENGINE = MergeTree() ORDER BY id",
        expect![[r#"
            CREATE TABLE t
            (
                id UInt64,
                INDEX idx id TYPE bloom_filter(0.01) GRANULARITY 1
            )
            ENGINE = MergeTree()
            ORDER BY id
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Column definition list: no extra blank lines
// ---------------------------------------------------------------------------

#[test]
fn column_def_list_no_extra_blanks() {
    check_format(
        "CREATE TABLE t (a UInt64, b String, c Float32) ENGINE = MergeTree() ORDER BY a",
        expect![[r#"
            CREATE TABLE t
            (
                a UInt64,
                b String,
                c Float32
            )
            ENGINE = MergeTree()
            ORDER BY a
        "#]],
    );
}

// ---------------------------------------------------------------------------
// Idempotency for new features
// ---------------------------------------------------------------------------

#[test]
fn idempotent_format_clause() {
    check_idempotent("SELECT 1 FORMAT JSON");
}

#[test]
fn format_clause_after_order_by() {
    check_format(
        "select col from t order by col format JSON",
        expect![[r#"
            SELECT
                col
            FROM t
            ORDER BY col
            FORMAT JSON
        "#]],
    );
}

#[test]
fn format_clause_after_group_by() {
    check_format(
        "select col from t group by col format JSON",
        expect![[r#"
            SELECT
                col
            FROM t
            GROUP BY col
            FORMAT JSON
        "#]],
    );
}

#[test]
fn format_clause_after_limit_by() {
    check_format(
        "select col from t limit 10 by col format JSON",
        expect![[r#"
            SELECT
                col
            FROM t
            LIMIT 10 BY col
            FORMAT JSON
        "#]],
    );
}

#[test]
fn format_clause_after_having() {
    check_format(
        "select col, count() from t group by col having count() > 1 format CSV",
        expect![[r#"
            SELECT
                col,
                count()
            FROM t
            GROUP BY col
            HAVING count() > 1
            FORMAT CSV
        "#]],
    );
}

#[test]
fn idempotent_codec_types() {
    check_idempotent("CREATE TABLE t (`ts` DateTime64(9) CODEC(Delta(8), ZSTD(1))) ENGINE = MergeTree() ORDER BY ts");
}

#[test]
fn idempotent_complex_ddl() {
    check_idempotent(
        "CREATE TABLE t
(
    `col` Map(LowCardinality(String), String) CODEC(ZSTD(1)),
    INDEX idx mapKeys(col) TYPE bloom_filter(0.01) GRANULARITY 1
)
ENGINE = SharedMergeTree('/path', '{replica}')
ORDER BY tuple()
SETTINGS index_granularity = 8192",
    );
}
