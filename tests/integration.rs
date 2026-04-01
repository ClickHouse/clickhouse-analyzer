use clickhouse_analyzer::{parse, SyntaxChild, SyntaxKind, SyntaxTree};
use expect_test::{expect, Expect};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively collect all token text from a syntax tree, preserving order.
/// If the CST is complete, this reconstructs the original input exactly.
fn collect_text(tree: &SyntaxTree) -> String {
    let mut buf = String::new();
    collect_text_rec(tree, &mut buf);
    buf
}

fn collect_text_rec(tree: &SyntaxTree, buf: &mut String) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => buf.push_str(&token.text),
            SyntaxChild::Tree(subtree) => collect_text_rec(subtree, buf),
        }
    }
}

/// Snapshot check: parse input and compare the printed tree.
fn check(input: &str, expected: Expect) {
    let result = parse(input);
    let mut buf = String::new();
    result.tree.print(&mut buf, 0);
    expected.assert_eq(&buf);
}

/// Snapshot check for errors: parse input and compare formatted error list.
fn check_errors(input: &str, expected: Expect) {
    let result = parse(input);
    let actual: String = result
        .errors
        .iter()
        .map(|e| format!("{}..{}: {}\n", e.range.0, e.range.1, e.message))
        .collect();
    expected.assert_eq(&actual);
}

// ===========================================================================
// 1. Structural invariants — must hold for ALL inputs
// ===========================================================================

#[test]
fn parser_never_panics_on_garbage() {
    let inputs = [
        "",
        " ",
        "\n",
        "\t",
        "\0",
        "!!!!",
        "))))",
        "((((",
        "[[[[",
        "]]]]",
        "{{{{",
        "}}}}",
        "SELECT",
        "FROM",
        "WHERE",
        "ORDER BY",
        &"(".repeat(200),
        &")".repeat(200),
        &"SELECT ".repeat(100),
        "SELECT 1 + + + + 2",
        "SELECT ,,,",
        "SELECT 'unclosed string",
        "SELECT \"unclosed identifier",
        "SELECT `unclosed backtick",
        ";;;;;;;",
        "SELECT * FROM (((())))",
        "SELECT 1 FROM db. WHERE",
        "SELECT INTERVAL",
        "SELECT INTERVAL INTERVAL INTERVAL",
        "SELECT :: :: ::",
        "SELECT -> -> ->",
        "SELECT 1 2 3 4 5",
        "SELECT [[[",
        "SELECT (((1)))",
        "SELECT 1; ; ; ; SELECT 2",
        "SELECT 1 FROM FROM FROM",
        "/* unclosed comment",
        "SELECT /* nested /* comment */ */",
        "SELECT 1 --line comment\nSELECT 2",
    ];
    for input in &inputs {
        let result = parse(input);
        assert_eq!(
            result.tree.kind,
            SyntaxKind::File,
            "Root should always be File for input: {:?}",
            input
        );
    }
}

#[test]
fn tree_covers_all_input_bytes() {
    let inputs = [
        "SELECT 1",
        "SELECT 1; SELECT 2",
        "  SELECT  1  ",
        "SELECT 1;\n",
        "",
        " ",
        "SELECT [1, 2, 3]",
        "SELECT INTERVAL 5 MINUTE",
        "SELECT now64();",
        ";;;",
        "SELECT 1 + 2 * 3",
        "SELECT a FROM t WHERE a > 1 ORDER BY a LIMIT 10",
        "SELECT x::Int32",
        "SELECT arrayMap(x -> x + 1, arr)",
        "SELECT (1, 2, 3)",
        "SELECT []",
        "SELECT ()",
        "-- just a comment\n",
        "/* block comment */",
        "SELECT 'hello world'",
        "SELECT \"quoted_id\"",
        // New statement types
        "INSERT INTO t VALUES (1, 2, 3)",
        "INSERT INTO t (a, b) SELECT 1, 2",
        "CREATE TABLE t (a Int32) ENGINE = MergeTree() ORDER BY a",
        "DROP TABLE IF EXISTS db.t ON CLUSTER c PERMANENTLY",
        "ALTER TABLE t ADD COLUMN c Int32, DROP COLUMN d",
        "DELETE FROM t WHERE x > 5",
        "EXPLAIN AST SELECT 1",
        "DESCRIBE TABLE db.t FORMAT JSON",
        "SHOW TABLES FROM mydb LIKE '%t%' LIMIT 10",
        "USE mydb",
        "SET max_threads = 4",
        "TRUNCATE TABLE IF EXISTS t",
        "RENAME TABLE old TO new",
        "EXISTS TABLE db.t",
        "CHECK TABLE t",
        "OPTIMIZE TABLE t FINAL DEDUPLICATE",
        "SELECT 1 UNION ALL SELECT 2",
        "SELECT a FROM t EXCEPT SELECT b FROM u",
    ];
    for input in &inputs {
        let result = parse(input);
        let reconstructed = collect_text(&result.tree);
        assert_eq!(
            &reconstructed, *input,
            "CST must reconstruct original input exactly"
        );
    }
}

#[test]
fn tree_covers_all_bytes_on_invalid_input() {
    let inputs = [
        "SELECT (1 + 2",
        "SELECT 1 FROM",
        "SELECT INTERVAL 5 POTATO",
        "!!! @@@ ###",
        "SELECT ,,,",
        "SELECT 'unclosed",
        "/* unclosed",
        "SELECT 1 2 3",
    ];
    for input in &inputs {
        let result = parse(input);
        let reconstructed = collect_text(&result.tree);
        assert_eq!(
            &reconstructed, *input,
            "CST must reconstruct original input even for invalid SQL: {:?}",
            input
        );
    }
}

#[test]
fn error_recovery_produces_errors_and_valid_tree() {
    let inputs = [
        "SELECT (1 + 2",
        "SELECT 1 FROM",
        "SELECT INTERVAL 5 POTATO",
        "SELECT (1 FROM",
        "SELECT 1 FROM db.",
    ];
    for input in &inputs {
        let result = parse(input);
        assert_eq!(result.tree.kind, SyntaxKind::File);
        assert!(
            !result.errors.is_empty(),
            "Should have errors for malformed input: {:?}",
            input
        );
    }
}

#[test]
fn valid_sql_produces_no_errors() {
    let inputs = [
        "SELECT 1",
        "SELECT 1;",
        "SELECT a FROM t",
        "SELECT a FROM t WHERE a > 1",
        "SELECT a FROM t ORDER BY a LIMIT 10",
        "SELECT now()",
        "SELECT [1, 2, 3]",
        "SELECT (1 + 2)",
        "SELECT x::Int32",
        "SELECT INTERVAL 5 MINUTE",
        "SELECT arrayMap(x -> x + 1, arr)",
        "SELECT quantile(0.9)(x)",
        "FROM t SELECT a",
        "SELECT 1; SELECT 2",
        "SELECT 1; SELECT 2;",
        "WITH a SELECT a",
        "SELECT []",
        "SELECT ()",
        // Keywords used as function names (ambiguous with JOIN keywords)
        "SELECT any(col) FROM t",
        "SELECT all(col) FROM t",
        "SELECT col_a a, any(col_b), any(col_c), count(*) d FROM t GROUP BY a ORDER BY d DESC LIMIT 10",
        // ANY/ALL as actual join keywords should still work
        "SELECT a FROM t ANY LEFT JOIN t2 ON t.a = t2.a",
        // INSERT
        "INSERT INTO t VALUES (1, 2, 3)",
        "INSERT INTO t (a, b) VALUES (1, 2)",
        "INSERT INTO db.t VALUES (1)",
        "INSERT INTO t SELECT 1, 2",
        "INSERT INTO t FORMAT JSONEachRow",
        "INSERT INTO TABLE t VALUES (1)",
        // CREATE TABLE
        "CREATE TABLE t (a Int32, b String) ENGINE = MergeTree() ORDER BY a",
        "CREATE TABLE IF NOT EXISTS db.t (a Int32) ENGINE = MergeTree() ORDER BY a",
        "CREATE TEMPORARY TABLE tmp (a Int32) ENGINE = Memory",
        "CREATE DATABASE IF NOT EXISTS mydb",
        "CREATE VIEW v AS SELECT 1",
        "CREATE MATERIALIZED VIEW mv TO dest AS SELECT 1 FROM t",
        "CREATE FUNCTION f AS (x) -> x + 1",
        // DROP / TRUNCATE / RENAME
        "DROP TABLE t",
        "DROP TABLE IF EXISTS db.t",
        "DROP DATABASE IF EXISTS mydb",
        "TRUNCATE TABLE t",
        "RENAME TABLE old TO new",
        "RENAME TABLE db.a TO db.b, db.c TO db.d",
        // USE / SET
        "USE mydb",
        "SET max_threads = 4",
        "SET max_threads = 4, max_memory_usage = 1000000",
        // EXISTS / CHECK / OPTIMIZE
        "EXISTS TABLE t",
        "EXISTS DATABASE mydb",
        "CHECK TABLE t",
        "OPTIMIZE TABLE t FINAL",
        "OPTIMIZE TABLE t PARTITION 202301 FINAL DEDUPLICATE",
        // ALTER
        "ALTER TABLE t ADD COLUMN c Int32",
        "ALTER TABLE t DROP COLUMN c",
        "ALTER TABLE t MODIFY COLUMN c String",
        "ALTER TABLE t DELETE WHERE x > 5",
        "ALTER TABLE t UPDATE x = 1 WHERE y > 0",
        // DELETE
        "DELETE FROM t WHERE x > 5",
        "DELETE FROM db.t WHERE x = 1",
        // EXPLAIN / DESCRIBE / SHOW
        "EXPLAIN AST SELECT 1",
        "EXPLAIN PLAN SELECT 1 FROM t",
        "DESCRIBE TABLE t",
        "DESC t",
        "SHOW TABLES",
        "SHOW TABLES FROM mydb LIKE '%t%'",
        "SHOW DATABASES",
        "SHOW CREATE TABLE t",
        "SHOW PROCESSLIST",
        // UNION / EXCEPT / INTERSECT
        "SELECT 1 UNION ALL SELECT 2",
        "SELECT 1 EXCEPT SELECT 2",
        "SELECT 1 INTERSECT SELECT 2",
        "SELECT a FROM t UNION ALL SELECT b FROM u UNION ALL SELECT c FROM v",
    ];
    for input in &inputs {
        let result = parse(input);
        assert!(
            result.errors.is_empty(),
            "Should have no errors for valid input {:?}, got: {:?}",
            input,
            result.errors
        );
    }
}

// ===========================================================================
// 2. Semicolons and multiple statements
// ===========================================================================

#[test]
fn trailing_semicolon() {
    check(
        "SELECT 1;",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
              ';'
        "#]],
    );
}

#[test]
fn trailing_semicolon_with_newline() {
    check_errors("SELECT 1;\n", expect![[""]]);
}

#[test]
fn multiple_statements() {
    check(
        "SELECT 1; SELECT 2",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
              ';'
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '2'
        "#]],
    );
}

#[test]
fn multiple_statements_trailing_semicolons() {
    check_errors("SELECT 1; SELECT 2;", expect![[""]]);
}

#[test]
fn empty_statements_between_semicolons() {
    check_errors("SELECT 1; ; ; SELECT 2", expect![[""]]);
}

#[test]
fn only_semicolons() {
    check(
        ";;;",
        expect![[r#"
            File
              ';'
              ';'
              ';'
        "#]],
    );
}

// ===========================================================================
// 3. Empty literals (recent fixes)
// ===========================================================================

#[test]
fn empty_array() {
    check(
        "SELECT []",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    ArrayExpression
                      '['
                      ']'
        "#]],
    );
}

#[test]
fn empty_parens() {
    check(
        "SELECT ()",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Expression
                      '('
                      ')'
        "#]],
    );
}

#[test]
fn empty_array_no_errors() {
    check_errors("SELECT []", expect![[""]]);
}

#[test]
fn empty_parens_no_errors() {
    check_errors("SELECT ()", expect![[""]]);
}

// ===========================================================================
// 4. Interval edge cases (recent fix)
// ===========================================================================

#[test]
fn interval_without_unit_at_eof() {
    check(
        "SELECT INTERVAL 5",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    IntervalExpression
                      'INTERVAL'
                      NumberLiteral
                        '5'
                      Error
        "#]],
    );
}

#[test]
fn interval_without_unit_before_from() {
    check(
        "SELECT INTERVAL 5 FROM t",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    IntervalExpression
                      'INTERVAL'
                      NumberLiteral
                        '5'
                      Error
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]],
    );
}

#[test]
fn interval_without_unit_does_not_eat_from() {
    // FROM must be parsed as a clause, not consumed as an interval unit error
    let result = parse("SELECT INTERVAL 5 FROM t");
    let mut buf = String::new();
    result.tree.print(&mut buf, 0);
    assert!(
        buf.contains("FromClause"),
        "FROM should be parsed as a clause, not consumed by interval error recovery"
    );
}

// ===========================================================================
// 5. Error recovery — tree structure after errors
// ===========================================================================

#[test]
fn unclosed_paren_recovery() {
    check(
        "SELECT (1 + 2",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    Expression
                      '('
                      BinaryExpression
                        NumberLiteral
                          '1'
                        '+'
                        NumberLiteral
                          '2'
        "#]],
    );
    check_errors(
        "SELECT (1 + 2",
        expect![[r#"
            13..13: expected )
        "#]],
    );
}

#[test]
fn missing_from_target_recovery() {
    check(
        "SELECT 1 FROM",
        expect![[r#"
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
                    Error
        "#]],
    );
}

#[test]
fn garbage_between_clauses_recovery() {
    check(
        "SELECT 1 POTATO FROM t",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    NumberLiteral
                      '1'
                    ColumnAlias
                      'POTATO'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]],
    );
}

#[test]
fn completely_invalid_input() {
    let result = parse("!!! @@@ ###");
    assert_eq!(result.tree.kind, SyntaxKind::File);
    assert!(!result.errors.is_empty());
    // CST must still cover all bytes
    let reconstructed = collect_text(&result.tree);
    assert_eq!(reconstructed, "!!! @@@ ###");
}

// ===========================================================================
// 6. Keywords as function names
// ===========================================================================

#[test]
fn any_as_function_name() {
    check(
        "SELECT any(col) FROM t",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    FunctionCall
                      Identifier
                        'any'
                      ExpressionList
                        '('
                        Expression
                          ColumnReference
                            'col'
                        ')'
                FromClause
                  'FROM'
                  TableIdentifier
                    't'
        "#]],
    );
}

#[test]
fn keyword_functions_in_select_with_aliases() {
    let sql = "SELECT col_a a, any(col_b), any(col_c), count(*) d FROM test_table GROUP BY a ORDER BY d DESC LIMIT 10";
    let result = parse(sql);
    assert!(
        result.errors.is_empty(),
        "Should have no errors for {:?}, got: {:?}",
        sql,
        result.errors
    );
}

#[test]
fn all_as_function_name() {
    let sql = "SELECT all(col) FROM t";
    let result = parse(sql);
    assert!(
        result.errors.is_empty(),
        "Should have no errors for {:?}, got: {:?}",
        sql,
        result.errors
    );
}

#[test]
fn any_as_join_keyword() {
    let sql = "SELECT a FROM t ANY LEFT JOIN t2 ON t.a = t2.a";
    let result = parse(sql);
    assert!(
        result.errors.is_empty(),
        "Should have no errors for {:?}, got: {:?}",
        sql,
        result.errors
    );
}

// ===========================================================================
// 7. Full integration smoke test (preserved from original)
// ===========================================================================

#[test]
fn test_full_parse() {
    let sql = "
        WITH
            a,
            b
        SELECT
            column_a,
            column_b,
            \"column c\",
            json.nested.path \"jsonNestedPath\",
            (SELECT sub_a FROM sub_table),
            (column_d + column_e) + column_f,
            testFunc(5)(column_g) + 5,
            (SELECT 1) + (SELECT 2 FROM system.\"numbers\") as subquery_result,
            my_int::Array(Tuple(Array(Int64), String)) casted_tuple,
            arrayMap((x, y) -> x + 1, (u, v) -> v + 1, [6, 7, 8, 9, (10), (SELECT 1 FROM system.numbers)]) \"array thing\"
        FROM table
        ORDER BY b;

        SELECT column_1;
        SELECT now() - INTERVAL 5 MINUTE;
        SELECT column, \"quoted column\", 'test', 3.14, 123;
        SELECT column_3 as c3, json.nested.path \"jsonNestedPath\" FROM table3;
        FROM system.numbers SELECT number WHERE number > 1 OR number < 5 AND 1=1 LIMIT 1;
    ";

    let result = parse(sql);
    let mut buf = String::new();
    result.tree.print(&mut buf, 0);
    assert!(buf.starts_with("File\n"));

    // CST completeness: every byte of input is in the tree
    let reconstructed = collect_text(&result.tree);
    assert_eq!(reconstructed, sql);
}
