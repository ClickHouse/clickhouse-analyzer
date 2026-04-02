use clickhouse_analyzer::{parse, SyntaxChild, SyntaxKind, SyntaxTree};
use expect_test::{expect, Expect};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively collect all token text from a syntax tree, preserving order.
/// If the CST is complete, this reconstructs the original input exactly.
fn collect_text(tree: &SyntaxTree, source: &str) -> String {
    let mut buf = String::new();
    collect_text_rec(tree, &mut buf, source);
    buf
}

fn collect_text_rec(tree: &SyntaxTree, buf: &mut String, source: &str) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => buf.push_str(token.text(source)),
            SyntaxChild::Tree(subtree) => collect_text_rec(subtree, buf, source),
        }
    }
}

/// Snapshot check: parse input and compare the printed tree.
fn check(input: &str, expected: Expect) {
    let result = parse(input);
    let mut buf = String::new();
    result.tree.print(&mut buf, 0, &result.source);
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

// ====================================================================// 1. Structural invariants — must hold for ALL inputs
// =============================================================#[test]
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
        // ATTACH / DETACH / EXCHANGE / UNDROP / BACKUP / RESTORE
        "ATTACH TABLE t",
        "DETACH TABLE IF EXISTS db.t PERMANENTLY",
        "EXCHANGE TABLES t1 AND t2 ON CLUSTER 'c'",
        "UNDROP TABLE db.t ON CLUSTER 'c'",
        "BACKUP TABLE db.t TO Disk('backups', '1.zip')",
        "RESTORE TABLE db.t FROM Disk('backups', '1.zip')",
        "GRANT SELECT ON db.t TO user1",
        "REVOKE ALL ON *.* FROM user1",
    ];
    for input in &inputs {
        let result = parse(input);
        let reconstructed = collect_text(&result.tree, &result.source);
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
        let reconstructed = collect_text(&result.tree, &result.source);
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
        // Query parameters in identifier positions
        "SELECT value FROM {database:Identifier}.{table:Identifier}",
        "SELECT {col:Identifier} FROM t",
        // Window functions
        "SELECT sum(x) OVER (PARTITION BY y ORDER BY z)",
        "SELECT sum(x) OVER (PARTITION BY y ORDER BY z ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)",
        "SELECT sum(x) OVER (ORDER BY z RANGE BETWEEN 1 PRECEDING AND 1 FOLLOWING)",
        "SELECT sum(x) OVER w",
        "SELECT sum(x) OVER w FROM t WINDOW w AS (PARTITION BY y ORDER BY z)",
        "SELECT sum(x) OVER (PARTITION BY y ORDER BY z ROWS UNBOUNDED PRECEDING)",
        // SAMPLE
        "SELECT * FROM t SAMPLE 0.1",
        "SELECT * FROM t SAMPLE 10000",
        "SELECT * FROM t SAMPLE 0.1 OFFSET 0.5",
        // ARRAY JOIN
        "SELECT * FROM t ARRAY JOIN arr1, arr2",
        "SELECT * FROM t LEFT ARRAY JOIN arr AS a",
        "SELECT * FROM t ARRAY JOIN arr1 AS a, arr2 AS b",
        // WITH FILL
        "SELECT date, value FROM t ORDER BY date WITH FILL STEP 1",
        "SELECT date, value FROM t ORDER BY date WITH FILL FROM 0 TO 100 STEP 1",
        // WINDOW clause with multiple definitions
        "SELECT sum(x) OVER w1, avg(y) OVER w2 FROM t WINDOW w1 AS (PARTITION BY a), w2 AS (ORDER BY b)",
        // ATTACH
        "ATTACH TABLE t",
        "ATTACH TABLE IF NOT EXISTS t",
        "ATTACH TABLE db.t ON CLUSTER 'cluster'",
        "ATTACH DATABASE db",
        // DETACH
        "DETACH TABLE t",
        "DETACH TABLE IF EXISTS t",
        "DETACH TABLE db.t ON CLUSTER 'cluster'",
        "DETACH TABLE db.t PERMANENTLY",
        "DETACH DATABASE db",
        // EXCHANGE
        "EXCHANGE TABLES t1 AND t2",
        "EXCHANGE TABLES db.t1 AND db.t2 ON CLUSTER 'cluster'",
        // UNDROP
        "UNDROP TABLE t",
        "UNDROP TABLE db.t ON CLUSTER 'cluster'",
        // BACKUP
        "BACKUP TABLE db.t TO Disk('backups', '1.zip')",
        "BACKUP DATABASE db TO Disk('backups', '1.zip')",
        // RESTORE
        "RESTORE TABLE db.t FROM Disk('backups', '1.zip')",
        "RESTORE DATABASE db FROM Disk('backups', '1.zip')",
        // GRANT
        "GRANT SELECT ON db.t TO user1",
        "GRANT SELECT, INSERT ON db.t TO user1, user2",
        "GRANT SELECT(col1, col2) ON db.t TO user1",
        "GRANT ALL ON *.* TO admin",
        "GRANT CREATE, ALTER, DROP ON db.* TO developer",
        "GRANT SELECT ON db.t TO user1 WITH GRANT OPTION",
        // REVOKE
        "REVOKE SELECT ON db.t FROM user1",
        "REVOKE ALL ON *.* FROM user1",
        "REVOKE ALL PRIVILEGES ON *.* FROM user1",
        // SYSTEM
        "SYSTEM RELOAD DICTIONARIES",
        "SYSTEM RELOAD DICTIONARY db.mydict",
        "SYSTEM RELOAD CONFIG",
        "SYSTEM RELOAD FUNCTIONS",
        "SYSTEM DROP DNS CACHE",
        "SYSTEM DROP MARK CACHE",
        "SYSTEM DROP UNCOMPRESSED CACHE",
        "SYSTEM DROP COMPILED CACHE",
        "SYSTEM FLUSH LOGS",
        "SYSTEM FLUSH DISTRIBUTED db.t",
        "SYSTEM STOP MERGES db.t",
        "SYSTEM START MERGES db.t",
        "SYSTEM STOP DISTRIBUTED SENDS db.t",
        "SYSTEM START DISTRIBUTED SENDS db.t",
        "SYSTEM STOP REPLICATED SENDS db.t",
        "SYSTEM START REPLICATED SENDS db.t",
        "SYSTEM STOP FETCHES db.t",
        "SYSTEM START FETCHES db.t",
        "SYSTEM STOP MOVES db.t",
        "SYSTEM START MOVES db.t",
        "SYSTEM SYNC REPLICA db.t",
        // KILL
        "KILL QUERY WHERE query_id = '123'",
        "KILL QUERY WHERE query_id = '123' SYNC",
        "KILL QUERY WHERE query_id = '123' ASYNC",
        "KILL QUERY WHERE query_id = '123' TEST",
        "KILL MUTATION WHERE mutation_id = '123'",
        // CREATE DICTIONARY with structured clauses
        "CREATE DICTIONARY mydict (id UInt64, name String) PRIMARY KEY id SOURCE(CLICKHOUSE(HOST 'localhost' PORT 9000 TABLE 'source_table' DB 'default')) LAYOUT(HASHED()) LIFETIME(300)",
        "CREATE DICTIONARY mydict (id UInt64, name String) PRIMARY KEY id SOURCE(CLICKHOUSE(HOST 'localhost')) LAYOUT(FLAT()) LIFETIME(MIN 300 MAX 600)",
        "CREATE DICTIONARY mydict (id UInt64, start Date, end Date) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(RANGE_HASHED()) RANGE(MIN start MAX end) LIFETIME(0)",
        "CREATE DICTIONARY mydict (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(COMPLEX_KEY_HASHED()) LIFETIME(300)",
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

// ====================================================================// 2. Semicolons and multiple statements
// =============================================================#[test]
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

// ====================================================================// 3. Empty literals (recent fixes)
// =============================================================#[test]
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

// ====================================================================// 4. Interval edge cases (recent fix)
// =============================================================#[test]
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
    result.tree.print(&mut buf, 0, &result.source);
    assert!(
        buf.contains("FromClause"),
        "FROM should be parsed as a clause, not consumed by interval error recovery"
    );
}

// ====================================================================// 5. Error recovery — tree structure after errors
// =============================================================#[test]
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
    let reconstructed = collect_text(&result.tree, &result.source);
    assert_eq!(reconstructed, "!!! @@@ ###");
}

// ====================================================================// 6. Keywords as function names
// =============================================================#[test]
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

// ====================================================================// 7. Full integration smoke test (preserved from original)
// =============================================================#[test]
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
    result.tree.print(&mut buf, 0, &result.source);
    assert!(buf.starts_with("File\n"));

    // CST completeness: every byte of input is in the tree
    let reconstructed = collect_text(&result.tree, &result.source);
    assert_eq!(reconstructed, sql);
}

// ====================================================================// Tuple element access via dot (e.g. expr.1, expr.2)
// =============================================================#[test]
fn tuple_dot_access_on_parenthesized_expr() {
    check(
        "SELECT (t).1",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    DotAccessExpression
                      Expression
                        '('
                        ColumnReference
                          't'
                        ')'
                      '.'
                      '1'
        "#]],
    );
}

#[test]
fn tuple_dot_access_with_alias_inside_parens() {
    check(
        "SELECT (func(a) AS t).1",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    DotAccessExpression
                      Expression
                        '('
                        FunctionCall
                          Identifier
                            'func'
                          ExpressionList
                            '('
                            Expression
                              ColumnReference
                                'a'
                            ')'
                        ColumnAlias
                          'AS'
                          't'
                        ')'
                      '.'
                      '1'
        "#]],
    );
}

#[test]
fn tuple_dot_access_in_binary_expr() {
    // (expr AS alias).1 + offset
    check_errors("SELECT (1 AS x).1 + 10", expect![[""]]);
}

#[test]
fn tuple_dot_access_field_name() {
    // Dot access with a named field instead of numeric index
    check_errors("SELECT (row AS r).name", expect![[""]]);
}

#[test]
fn dot_access_on_function_call() {
    check(
        "SELECT func(a, b).1",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    DotAccessExpression
                      FunctionCall
                        Identifier
                          'func'
                        ExpressionList
                          '('
                          Expression
                            ColumnReference
                              'a'
                          ','
                          Expression
                            ColumnReference
                              'b'
                          ')'
                      '.'
                      '1'
        "#]],
    );
}

#[test]
fn chained_dot_access() {
    // (expr).field1.field2 — dot access chains
    check_errors("SELECT (t).a.b", expect![[""]]);
}

#[test]
fn multiple_aliased_exprs_in_tuple() {
    // (expr AS a, expr AS b) — tuple with aliases
    check_errors("SELECT (1 AS a, 2 AS b)", expect![[""]]);
}

#[test]
fn expression_alias_in_with_clause() {
    // WITH expr AS alias — standard WITH alias usage
    check_errors("WITH 1 AS x SELECT x + 1", expect![[""]]);
}

#[test]
fn named_tuple_cast_dot_access() {
    // Cast to named Tuple then access field by name via dot
    check(
        "SELECT ('a', 'b')::Tuple(x String, y String).x",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    DotAccessExpression
                      CastExpression
                        TupleExpression
                          '('
                          StringLiteral
                            ''a''
                          ','
                          StringLiteral
                            ''b''
                          ')'
                        '::'
                        DataType
                          'Tuple'
                          DataTypeParameters
                            '('
                            'x'
                            DataType
                              'String'
                            ','
                            'y'
                            DataType
                              'String'
                            ')'
                      '.'
                      'x'
        "#]],
    );
}

// ====================================================================// Parenthesized subquery in CREATE VIEW AS clause
// =============================================================#[test]
fn create_view_as_parenthesized_subquery() {
    check(
        "CREATE VIEW v AS (SELECT 1)",
        expect![[r#"
            File
              CreateStatement
                'CREATE'
                ViewDefinition
                  'VIEW'
                  TableIdentifier
                    'v'
                  AsClause
                    'AS'
                    '('
                    SelectStatement
                      SelectClause
                        'SELECT'
                        ColumnList
                          NumberLiteral
                            '1'
                    ')'
        "#]],
    );
}

#[test]
fn create_view_as_parenthesized_subquery_complex() {
    check_errors(
        "CREATE VIEW IF NOT EXISTS db.v AS (SELECT a, b FROM t WHERE x > 1)",
        expect![[""]],
    );
}

// ====================================================================// Modulo operator
// =============================================================#[test]
fn modulo_operator() {
    check(
        "SELECT a % 5",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    BinaryExpression
                      ColumnReference
                        'a'
                      '%'
                      NumberLiteral
                        '5'
        "#]],
    );
}

#[test]
fn modulo_precedence_same_as_multiply() {
    // % binds at the same level as * and /
    check_errors("SELECT a + b % c * d", expect![[""]]);
}

// ====================================================================// Cast (::) on arbitrary expressions
// =============================================================#[test]
fn cast_on_function_call() {
    check(
        "SELECT func(x)::UInt32",
        expect![[r#"
            File
              SelectStatement
                SelectClause
                  'SELECT'
                  ColumnList
                    CastExpression
                      FunctionCall
                        Identifier
                          'func'
                        ExpressionList
                          '('
                          Expression
                            ColumnReference
                              'x'
                          ')'
                      '::'
                      DataType
                        'UInt32'
        "#]],
    );
}

#[test]
fn cast_then_dot_access() {
    // Cast to named tuple type, then access field
    check_errors("SELECT ('a', 'b')::Tuple(x String, y String).x", expect![[""]]);
}

#[test]
fn cast_then_array_access() {
    check_errors("SELECT func(x)::Array(UInt32)[0]", expect![[""]]);
}

#[test]
fn cast_on_parenthesized_expr() {
    check_errors("SELECT (a + b)::Float64", expect![[""]]);
}

// ====================================================================// Full integration tests
// =============================================================#[test]
fn materialized_view_with_dot_access() {
    // Full integration: MV with expression alias + tuple dot access
    let sql = "\
        CREATE MATERIALIZED VIEW db.mv TO db.target AS \
        WITH func(col) AS t \
        SELECT (arrayJoin(arr) AS item).1 AS idx, item.2 AS val \
        FROM db.src";
    let result = parse(sql);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let reconstructed = collect_text(&result.tree, &result.source);
    assert_eq!(reconstructed, sql);
}

// ====================================================================// Error recovery tests
// =============================================================#[test]
fn recovery_misspelled_where() {
    // WHER should not cause cascading errors
    let result = parse("SELECT 1 FROM t WHER x > 1");
    // Should have errors but not many "Unexpected token" errors
    assert!(result.errors.len() <= 4, "Too many errors: {:?}", result.errors);
    // Tree should cover all bytes
    assert_eq!(collect_text(&result.tree, &result.source), "SELECT 1 FROM t WHER x > 1");
}

#[test]
fn recovery_misspelled_engine() {
    let result = parse("CREATE TABLE t (id UInt64) ENIGNE = MergeTree() ORDER BY id");
    assert_eq!(collect_text(&result.tree, &result.source), "CREATE TABLE t (id UInt64) ENIGNE = MergeTree() ORDER BY id");
    // Should still parse something after ENIGNE, not eat ORDER BY as garbage
}

#[test]
fn recovery_garbage_between_create_clauses() {
    let result = parse("CREATE TABLE t (id UInt64) ENGINE = MergeTree() GARBAGE ORDER BY id");
    assert_eq!(collect_text(&result.tree, &result.source), "CREATE TABLE t (id UInt64) ENGINE = MergeTree() GARBAGE ORDER BY id");
    // GARBAGE should be an error, ORDER BY should still parse
}

#[test]
fn recovery_misspelled_from_in_show() {
    let result = parse("SHOW TABLES FORM default");
    assert_eq!(collect_text(&result.tree, &result.source), "SHOW TABLES FORM default");
}

// ====================================================================// Query parameters in identifier positions
// =============================================================#[test]
fn query_parameter_as_table_identifier() {
    check("SELECT value FROM {database:Identifier}.{table:Identifier}", expect![[r#"
        File
          SelectStatement
            SelectClause
              'SELECT'
              ColumnList
                ColumnReference
                  'value'
            FromClause
              'FROM'
              TableIdentifier
                QueryParameterExpression
                  '{'
                  'database'
                  ':'
                  DataType
                    'Identifier'
                  '}'
                '.'
                QueryParameterExpression
                  '{'
                  'table'
                  ':'
                  DataType
                    'Identifier'
                  '}'
    "#]]);
}

#[test]
fn query_parameter_as_single_table() {
    check("SELECT 1 FROM {tbl:Identifier}", expect![[r#"
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
                QueryParameterExpression
                  '{'
                  'tbl'
                  ':'
                  DataType
                    'Identifier'
                  '}'
    "#]]);
}

#[test]
fn query_parameter_mixed_with_bare_identifier() {
    check("SELECT 1 FROM {db:Identifier}.my_table", expect![[r#"
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
                QueryParameterExpression
                  '{'
                  'db'
                  ':'
                  DataType
                    'Identifier'
                  '}'
                '.'
                'my_table'
    "#]]);
}

// ====================================================================// ATTACH / DETACH / EXCHANGE / UNDROP / BACKUP / RESTORE statements
// =============================================================#[test]
fn attach_table() {
    check(
        "ATTACH TABLE t",
        expect![[r#"
            File
              AttachStatement
                'ATTACH'
                'TABLE'
                TableIdentifier
                  't'
        "#]],
    );
}

#[test]
fn attach_table_if_not_exists() {
    check(
        "ATTACH TABLE IF NOT EXISTS t",
        expect![[r#"
            File
              AttachStatement
                'ATTACH'
                'TABLE'
                IfNotExistsClause
                  'IF'
                  'NOT'
                  'EXISTS'
                TableIdentifier
                  't'
        "#]],
    );
}

#[test]
fn attach_table_on_cluster() {
    check(
        "ATTACH TABLE db.t ON CLUSTER 'cluster'",
        expect![[r#"
            File
              AttachStatement
                'ATTACH'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                OnClusterClause
                  'ON'
                  'CLUSTER'
                  ''cluster''
        "#]],
    );
}

#[test]
fn attach_database() {
    check(
        "ATTACH DATABASE db",
        expect![[r#"
            File
              AttachStatement
                'ATTACH'
                'DATABASE'
                TableIdentifier
                  'db'
        "#]],
    );
}

#[test]
fn detach_table() {
    check(
        "DETACH TABLE t",
        expect![[r#"
            File
              DetachStatement
                'DETACH'
                'TABLE'
                TableIdentifier
                  't'
        "#]],
    );
}

#[test]
fn detach_table_if_exists() {
    check(
        "DETACH TABLE IF EXISTS t",
        expect![[r#"
            File
              DetachStatement
                'DETACH'
                'TABLE'
                IfExistsClause
                  'IF'
                  'EXISTS'
                TableIdentifier
                  't'
        "#]],
    );
}

#[test]
fn detach_table_on_cluster() {
    check(
        "DETACH TABLE db.t ON CLUSTER 'cluster'",
        expect![[r#"
            File
              DetachStatement
                'DETACH'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                OnClusterClause
                  'ON'
                  'CLUSTER'
                  ''cluster''
        "#]],
    );
}

#[test]
fn detach_table_permanently() {
    check(
        "DETACH TABLE db.t PERMANENTLY",
        expect![[r#"
            File
              DetachStatement
                'DETACH'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                'PERMANENTLY'
        "#]],
    );
}

#[test]
fn detach_database() {
    check(
        "DETACH DATABASE db",
        expect![[r#"
            File
              DetachStatement
                'DETACH'
                'DATABASE'
                TableIdentifier
                  'db'
        "#]],
    );
}

#[test]
fn exchange_tables() {
    check(
        "EXCHANGE TABLES t1 AND t2",
        expect![[r#"
            File
              ExchangeStatement
                'EXCHANGE'
                'TABLES'
                TableIdentifier
                  't1'
                'AND'
                TableIdentifier
                  't2'
        "#]],
    );
}

#[test]
fn exchange_tables_on_cluster() {
    check(
        "EXCHANGE TABLES db.t1 AND db.t2 ON CLUSTER 'cluster'",
        expect![[r#"
            File
              ExchangeStatement
                'EXCHANGE'
                'TABLES'
                TableIdentifier
                  'db'
                  '.'
                  't1'
                'AND'
                TableIdentifier
                  'db'
                  '.'
                  't2'
                OnClusterClause
                  'ON'
                  'CLUSTER'
                  ''cluster''
        "#]],
    );
}

#[test]
fn undrop_table() {
    check(
        "UNDROP TABLE t",
        expect![[r#"
            File
              UndropStatement
                'UNDROP'
                'TABLE'
                TableIdentifier
                  't'
        "#]],
    );
}

#[test]
fn undrop_table_on_cluster() {
    check(
        "UNDROP TABLE db.t ON CLUSTER 'cluster'",
        expect![[r#"
            File
              UndropStatement
                'UNDROP'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                OnClusterClause
                  'ON'
                  'CLUSTER'
                  ''cluster''
        "#]],
    );
}

#[test]
fn backup_table() {
    check(
        "BACKUP TABLE db.t TO Disk('backups', '1.zip')",
        expect![[r#"
            File
              BackupStatement
                'BACKUP'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                'TO'
                FunctionCall
                  Identifier
                    'Disk'
                  ExpressionList
                    '('
                    Expression
                      StringLiteral
                        ''backups''
                    ','
                    Expression
                      StringLiteral
                        ''1.zip''
                    ')'
        "#]],
    );
}

#[test]
fn backup_database() {
    check(
        "BACKUP DATABASE db TO Disk('backups', '1.zip')",
        expect![[r#"
            File
              BackupStatement
                'BACKUP'
                'DATABASE'
                TableIdentifier
                  'db'
                'TO'
                FunctionCall
                  Identifier
                    'Disk'
                  ExpressionList
                    '('
                    Expression
                      StringLiteral
                        ''backups''
                    ','
                    Expression
                      StringLiteral
                        ''1.zip''
                    ')'
        "#]],
    );
}

#[test]
fn backup_with_settings() {
    check(
        "BACKUP TABLE db.t TO Disk('backups', '1.zip') SETTINGS base_backup = Disk('backups', '0.zip')",
        expect![[r#"
            File
              BackupStatement
                'BACKUP'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                'TO'
                FunctionCall
                  Identifier
                    'Disk'
                  ExpressionList
                    '('
                    Expression
                      StringLiteral
                        ''backups''
                    ','
                    Expression
                      StringLiteral
                        ''1.zip''
                    ')'
                SettingsClause
                  'SETTINGS'
                  SettingItem
                    'base_backup'
                    '='
                    FunctionCall
                      Identifier
                        'Disk'
                      ExpressionList
                        '('
                        Expression
                          StringLiteral
                            ''backups''
                        ','
                        Expression
                          StringLiteral
                            ''0.zip''
                        ')'
        "#]],
    );
}

#[test]
fn restore_table() {
    check(
        "RESTORE TABLE db.t FROM Disk('backups', '1.zip')",
        expect![[r#"
            File
              RestoreStatement
                'RESTORE'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                'FROM'
                FunctionCall
                  Identifier
                    'Disk'
                  ExpressionList
                    '('
                    Expression
                      StringLiteral
                        ''backups''
                    ','
                    Expression
                      StringLiteral
                        ''1.zip''
                    ')'
        "#]],
    );
}

#[test]
fn grant_all_on_wildcard() {
    check(
        "GRANT ALL ON *.* TO admin",
        expect![[r#"
            File
              GrantStatement
                'GRANT'
                PrivilegeList
                  Privilege
                    'ALL'
                GrantTarget
                  'ON'
                  '*'
                  '.'
                  '*'
                'TO'
                'admin'
        "#]],
    );
}

#[test]
fn grant_on_db_wildcard() {
    check(
        "GRANT CREATE, ALTER, DROP ON db.* TO developer",
        expect![[r#"
            File
              GrantStatement
                'GRANT'
                PrivilegeList
                  Privilege
                    'CREATE'
                  ','
                  Privilege
                    'ALTER'
                  ','
                  Privilege
                    'DROP'
                GrantTarget
                  'ON'
                  'db'
                  '.'
                  '*'
                'TO'
                'developer'
        "#]],
    );
}

#[test]
fn grant_with_grant_option() {
    check(
        "GRANT SELECT ON db.t TO user1 WITH GRANT OPTION",
        expect![[r#"
            File
              GrantStatement
                'GRANT'
                PrivilegeList
                  Privilege
                    'SELECT'
                GrantTarget
                  'ON'
                  'db'
                  '.'
                  't'
                'TO'
                'user1'
                'WITH'
                'GRANT'
                'OPTION'
        "#]],
    );
}

#[test]
fn revoke_simple() {
    check(
        "REVOKE SELECT ON db.t FROM user1",
        expect![[r#"
            File
              RevokeStatement
                'REVOKE'
                PrivilegeList
                  Privilege
                    'SELECT'
                GrantTarget
                  'ON'
                  'db'
                  '.'
                  't'
                'FROM'
                'user1'
        "#]],
    );
}

#[test]
fn restore_database() {
    check(
        "RESTORE DATABASE db FROM Disk('backups', '1.zip')",
        expect![[r#"
            File
              RestoreStatement
                'RESTORE'
                'DATABASE'
                TableIdentifier
                  'db'
                'FROM'
                FunctionCall
                  Identifier
                    'Disk'
                  ExpressionList
                    '('
                    Expression
                      StringLiteral
                        ''backups''
                    ','
                    Expression
                      StringLiteral
                        ''1.zip''
                    ')'
        "#]],
    );
}

#[test]
fn restore_with_settings() {
    check(
        "RESTORE TABLE db.t FROM Disk('backups', '1.zip') SETTINGS allow_non_empty_tables = true",
        expect![[r#"
            File
              RestoreStatement
                'RESTORE'
                'TABLE'
                TableIdentifier
                  'db'
                  '.'
                  't'
                'FROM'
                FunctionCall
                  Identifier
                    'Disk'
                  ExpressionList
                    '('
                    Expression
                      StringLiteral
                        ''backups''
                    ','
                    Expression
                      StringLiteral
                        ''1.zip''
                    ')'
                SettingsClause
                  'SETTINGS'
                  SettingItem
                    'allow_non_empty_tables'
                    '='
                    BooleanLiteral
                      'true'
        "#]],
    );
}

#[test]
fn revoke_all_privileges() {
    check(
        "REVOKE ALL PRIVILEGES ON *.* FROM user1",
        expect![[r#"
            File
              RevokeStatement
                'REVOKE'
                PrivilegeList
                  Privilege
                    'ALL'
                    'PRIVILEGES'
                GrantTarget
                  'ON'
                  '*'
                  '.'
                  '*'
                'FROM'
                'user1'
        "#]],
    );
}

// ====================================================================
// Dictionary DDL deep parsing
// ====================================================================

#[test]
fn dictionary_source_layout_lifetime() {
    check(
        "CREATE DICTIONARY d (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE(HOST 'localhost' PORT 9000)) LAYOUT(HASHED()) LIFETIME(300)",
        expect![[r#"
            File
              CreateStatement
                'CREATE'
                DictionaryDefinition
                  'DICTIONARY'
                  TableIdentifier
                    'd'
                  ColumnDefinitionList
                    '('
                    ColumnDefinition
                      'id'
                      DataType
                        'UInt64'
                    ')'
                  PrimaryKeyDefinition
                    'PRIMARY'
                    'KEY'
                    ColumnReference
                      'id'
                  DictionarySource
                    'SOURCE'
                    '('
                    DictionarySourceType
                      'CLICKHOUSE'
                      '('
                      DictionaryKeyValue
                        'HOST'
                        ''localhost''
                      DictionaryKeyValue
                        'PORT'
                        '9000'
                      ')'
                    ')'
                  DictionaryLayout
                    'LAYOUT'
                    '('
                    DictionaryLayoutType
                      'HASHED'
                      '('
                      ')'
                    ')'
                  DictionaryLifetime
                    'LIFETIME'
                    '('
                    '300'
                    ')'
        "#]],
    );
}

#[test]
fn dictionary_lifetime_min_max() {
    check(
        "CREATE DICTIONARY d (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(FLAT()) LIFETIME(MIN 300 MAX 600)",
        expect![[r#"
            File
              CreateStatement
                'CREATE'
                DictionaryDefinition
                  'DICTIONARY'
                  TableIdentifier
                    'd'
                  ColumnDefinitionList
                    '('
                    ColumnDefinition
                      'id'
                      DataType
                        'UInt64'
                    ')'
                  PrimaryKeyDefinition
                    'PRIMARY'
                    'KEY'
                    ColumnReference
                      'id'
                  DictionarySource
                    'SOURCE'
                    '('
                    DictionarySourceType
                      'CLICKHOUSE'
                      '('
                      ')'
                    ')'
                  DictionaryLayout
                    'LAYOUT'
                    '('
                    DictionaryLayoutType
                      'FLAT'
                      '('
                      ')'
                    ')'
                  DictionaryLifetime
                    'LIFETIME'
                    '('
                    DictionaryKeyValue
                      'MIN'
                      '300'
                    DictionaryKeyValue
                      'MAX'
                      '600'
                    ')'
        "#]],
    );
}

#[test]
fn dictionary_range_clause() {
    check(
        "CREATE DICTIONARY d (id UInt64, start Date, end Date) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(RANGE_HASHED()) RANGE(MIN start MAX end) LIFETIME(0)",
        expect![[r#"
            File
              CreateStatement
                'CREATE'
                DictionaryDefinition
                  'DICTIONARY'
                  TableIdentifier
                    'd'
                  ColumnDefinitionList
                    '('
                    ColumnDefinition
                      'id'
                      DataType
                        'UInt64'
                    ','
                    ColumnDefinition
                      'start'
                      DataType
                        'Date'
                    ','
                    ColumnDefinition
                      'end'
                      DataType
                        'Date'
                    ')'
                  PrimaryKeyDefinition
                    'PRIMARY'
                    'KEY'
                    ColumnReference
                      'id'
                  DictionarySource
                    'SOURCE'
                    '('
                    DictionarySourceType
                      'CLICKHOUSE'
                      '('
                      ')'
                    ')'
                  DictionaryLayout
                    'LAYOUT'
                    '('
                    DictionaryLayoutType
                      'RANGE_HASHED'
                      '('
                      ')'
                    ')'
                  DictionaryRange
                    'RANGE'
                    '('
                    DictionaryKeyValue
                      'MIN'
                      'start'
                    DictionaryKeyValue
                      'MAX'
                      'end'
                    ')'
                  DictionaryLifetime
                    'LIFETIME'
                    '('
                    '0'
                    ')'
        "#]],
    );
}
