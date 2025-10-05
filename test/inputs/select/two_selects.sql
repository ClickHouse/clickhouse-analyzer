WITH
    a,
    b
SELECT
    column_a,
    column_b,
    "column c",
    json.nested.path "jsonNestedPath",
    (SELECT sub_a FROM sub_table),
    (column_d + column_e) + column_f,
    testFunc(5)(column_g) + 5,
    (SELECT 1) + (SELECT 2 FROM system."numbers") as subquery_result,
    my_int::Array(Tuple(Array(Int64), String)) casted_tuple,
    arrayMap((x, y) -> x + 1, (u, v) -> v + 1, [6, 7, 8, 9, (10), (SELECT 1 FROM system.numbers)]) "array thing"
FROM table
ORDER BY b;

SELECT column_1;
SELECT column, "quoted column", 'test', 3.14, 123;
SELECT column_3 as c3, json.nested.path "jsonNestedPath" FROM table3;
FROM system.numbers SELECT number WHERE number > 1 OR number < 5 AND 1=1 LIMIT 1;