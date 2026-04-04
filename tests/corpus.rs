//! Validate the parser against the ClickHouse test query corpus.
//!
//! This test parses all .sql files from the ClickHouse repo's test suite
//! and reports coverage. It does NOT fail on parse errors — it reports them.
//!
//! Run with:
//!   CLICKHOUSE_QUERIES_PATH=/Users/al/ch/ClickHouse/tests/queries \
//!     cargo test --test corpus -- --nocapture

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use clickhouse_analyzer::parse;

fn get_corpus_path() -> Option<PathBuf> {
    std::env::var("CLICKHOUSE_QUERIES_PATH")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

#[test]
fn parse_clickhouse_test_corpus() {
    let Some(corpus_path) = get_corpus_path() else {
        eprintln!("CLICKHOUSE_QUERIES_PATH not set or does not exist, skipping corpus test");
        return;
    };

    let sql_files: Vec<PathBuf> = walkdir::WalkDir::new(&corpus_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "sql")
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    eprintln!("Found {} .sql files in {}", sql_files.len(), corpus_path.display());

    let mut total_files = 0;
    let mut clean_files = 0;
    let mut total_statements = 0u64;
    let mut clean_statements = 0u64;
    let mut error_categories: HashMap<String, usize> = HashMap::new();
    let mut failing_files: Vec<(PathBuf, usize)> = Vec::new();

    for path in &sql_files {
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Skip files that are clearly not pure SQL (shell scripts, etc.)
        if source.starts_with("#!") || source.starts_with("#!/") {
            continue;
        }

        total_files += 1;

        // Split by semicolons for rough statement count
        // (not perfect but good enough for coverage metrics)
        let result = parse(&source);
        let errors = &result.errors;

        let stmt_count = source
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .count() as u64;
        total_statements += stmt_count;

        if errors.is_empty() {
            clean_files += 1;
            clean_statements += stmt_count;
        } else {
            failing_files.push((path.clone(), errors.len()));
            for err in errors {
                // Categorize by first few words of the error message
                let category = err
                    .message
                    .split_whitespace()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" ");
                *error_categories.entry(category).or_insert(0) += 1;
            }
        }
    }

    let file_pct = if total_files > 0 {
        clean_files as f64 / total_files as f64 * 100.0
    } else {
        0.0
    };
    let stmt_pct = if total_statements > 0 {
        clean_statements as f64 / total_statements as f64 * 100.0
    } else {
        0.0
    };

    eprintln!("\n=== Corpus Parse Coverage ===");
    eprintln!("Files:      {clean_files}/{total_files} ({file_pct:.1}%)");
    eprintln!("Statements: {clean_statements}/{total_statements} (est. {stmt_pct:.1}%)");
    eprintln!("Files with errors: {}", failing_files.len());

    // Top error categories
    let mut sorted_errors: Vec<_> = error_categories.into_iter().collect();
    sorted_errors.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("\n--- Top error categories ---");
    for (category, count) in sorted_errors.iter().take(20) {
        eprintln!("  {count:5}  {category}");
    }

    // Files with most errors
    failing_files.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("\n--- Files with most errors ---");
    for (path, count) in failing_files.iter().take(15) {
        let relative = path.strip_prefix(&corpus_path).unwrap_or(path);
        eprintln!("  {count:3} errors  {}", relative.display());
    }

    // This is informational — we don't fail the test.
    // The coverage percentage serves as a baseline to track improvement.
    eprintln!("\n(This test is informational and does not fail on parse errors)");
}
