use clickhouse_analyzer::{format, parse, FormatConfig};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const SMALL_QUERY: &str = "SELECT 1";

const MEDIUM_QUERY: &str = "\
SELECT a, b, c \
FROM t1 \
INNER JOIN t2 ON t1.id = t2.id \
WHERE a > 0 \
GROUP BY a \
ORDER BY b \
LIMIT 100";

const LARGE_QUERY: &str = "\
WITH
  regional_sales AS (
    SELECT region, SUM(amount) AS total_sales
    FROM orders
    WHERE order_date >= '2024-01-01' AND order_date < '2025-01-01'
    GROUP BY region
    HAVING SUM(amount) > 1000000
  ),
  top_regions AS (
    SELECT region
    FROM regional_sales
    WHERE total_sales > (SELECT SUM(total_sales) / 10 FROM regional_sales)
  )
SELECT
  o.region,
  o.product,
  SUM(o.quantity) AS product_units,
  SUM(o.amount) AS product_sales,
  COUNT(DISTINCT o.customer_id) AS unique_customers
FROM orders AS o
INNER JOIN top_regions AS tr ON o.region = tr.region
LEFT JOIN products AS p ON o.product_id = p.id
WHERE o.order_date >= '2024-01-01'
  AND o.status IN ('completed', 'shipped', 'delivered')
  AND p.category != 'discontinued'
GROUP BY o.region, o.product
HAVING SUM(o.amount) > 10000
ORDER BY o.region ASC, product_sales DESC
LIMIT 100
SETTINGS max_threads = 4";

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    group.bench_function("small", |b| {
        b.iter(|| parse(black_box(SMALL_QUERY)))
    });

    group.bench_function("medium", |b| {
        b.iter(|| parse(black_box(MEDIUM_QUERY)))
    });

    group.bench_function("large", |b| {
        b.iter(|| parse(black_box(LARGE_QUERY)))
    });

    group.finish();
}

fn bench_parse_and_format(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_and_format");
    let config = FormatConfig::default();

    group.bench_function("small", |b| {
        b.iter(|| {
            let result = parse(black_box(SMALL_QUERY));
            format(&result.tree, &config, &result.source)
        })
    });

    group.bench_function("medium", |b| {
        b.iter(|| {
            let result = parse(black_box(MEDIUM_QUERY));
            format(&result.tree, &config, &result.source)
        })
    });

    group.bench_function("large", |b| {
        b.iter(|| {
            let result = parse(black_box(LARGE_QUERY));
            format(&result.tree, &config, &result.source)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_parse, bench_parse_and_format);
criterion_main!(benches);
