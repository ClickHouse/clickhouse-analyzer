#![no_main]
use clickhouse_analyzer::{format, parse, FormatConfig, SyntaxChild, SyntaxKind, SyntaxTree};
use libfuzzer_sys::fuzz_target;

/// Recursively collect all token text from a syntax tree.
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

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Parse must not panic and root must be File
    let result = parse(input);
    assert_eq!(result.tree.kind, SyntaxKind::File);

    // CST must cover all input bytes
    let reconstructed = collect_text(&result.tree, &result.source);
    assert_eq!(reconstructed, input);

    // Formatting must not panic
    let config = FormatConfig::default();
    let _ = format(&result.tree, &config, &result.source);
});
