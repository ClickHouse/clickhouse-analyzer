use clickhouse_analyzer::{format, parse, FormatConfig, SyntaxChild, SyntaxKind, SyntaxTree};
use proptest::prelude::*;

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

/// Collect all leaf tokens from the tree in order.
fn collect_tokens(tree: &SyntaxTree) -> Vec<(u32, u32)> {
    let mut tokens = Vec::new();
    collect_tokens_rec(tree, &mut tokens);
    tokens
}

fn collect_tokens_rec(tree: &SyntaxTree, tokens: &mut Vec<(u32, u32)>) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => tokens.push((token.start, token.end)),
            SyntaxChild::Tree(subtree) => collect_tokens_rec(subtree, tokens),
        }
    }
}

/// Check that all tree node ranges are consistent (children within parent).
fn assert_ranges_consistent(tree: &SyntaxTree) {
    for child in &tree.children {
        if let SyntaxChild::Tree(subtree) = child {
            assert!(
                subtree.start <= subtree.end,
                "Node {:?} has start {} > end {}",
                subtree.kind,
                subtree.start,
                subtree.end
            );
            // Children should be within parent range (unless parent is empty sentinel)
            if tree.start != u32::MAX && !tree.children.is_empty() {
                assert!(
                    subtree.start >= tree.start,
                    "Child {:?} start {} < parent {:?} start {}",
                    subtree.kind,
                    subtree.start,
                    tree.kind,
                    tree.start
                );
                assert!(
                    subtree.end <= tree.end,
                    "Child {:?} end {} > parent {:?} end {}",
                    subtree.kind,
                    subtree.end,
                    tree.kind,
                    tree.end
                );
            }
            assert_ranges_consistent(subtree);
        }
    }
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Parse never panics on arbitrary ASCII strings, and root is always File.
    #[test]
    fn parse_never_panics(input in "[\\x00-\\x7F]{0,200}") {
        let result = parse(&input);
        prop_assert_eq!(
            result.tree.kind,
            SyntaxKind::File,
            "Root should always be File for input: {:?}",
            input
        );
    }

    /// The CST covers all input bytes: collecting token text reconstructs the
    /// original input exactly.
    #[test]
    fn cst_covers_all_input(input in "[\\x00-\\x7F]{0,200}") {
        let result = parse(&input);
        let reconstructed = collect_text(&result.tree, &result.source);
        prop_assert_eq!(
            reconstructed,
            input,
            "Reconstructed text does not match original input"
        );
    }

    /// Formatter idempotency: format(parse(format(parse(x)))) == format(parse(x)).
    /// Tests with well-formed SQL queries to verify the formatter stabilizes.
    #[test]
    fn formatter_idempotent(
        cols in prop::collection::vec("[a-z][a-z0-9_]{0,5}", 1..5),
        table in "[a-z][a-z0-9_]{0,8}",
        has_where in proptest::bool::ANY,
        limit in prop::option::of(1u32..1000),
    ) {
        let col_list = cols.join(", ");
        let mut query = std::format!("SELECT {} FROM {}", col_list, table);
        if has_where {
            query.push_str(" WHERE x > 0");
        }
        if let Some(n) = limit {
            query.push_str(&std::format!(" LIMIT {}", n));
        }
        let input = query;
        let config = FormatConfig::default();

        let result1 = parse(&input);
        let formatted1 = format(&result1.tree, &config, &result1.source);

        let result2 = parse(&formatted1);
        let formatted2 = format(&result2.tree, &config, &result2.source);

        prop_assert_eq!(
            formatted1,
            formatted2,
            "Formatter is not idempotent for input: {:?}",
            input
        );
    }

    /// Formatter idempotency on arbitrary (possibly malformed) input.
    /// The formatter must stabilize in one pass even on garbage SQL.
    #[test]
    fn formatter_idempotent_arbitrary(input in "[\\x00-\\x7F]{0,200}") {
        let config = FormatConfig::default();

        let result1 = parse(&input);
        let formatted1 = format(&result1.tree, &config, &result1.source);

        let result2 = parse(&formatted1);
        let formatted2 = format(&result2.tree, &config, &result2.source);

        prop_assert_eq!(
            formatted1,
            formatted2,
            "Formatter is not idempotent for arbitrary input: {:?}",
            input
        );
    }

    /// Token spans are contiguous: they form a covering of 0..source.len()
    /// with no gaps and no overlaps.
    #[test]
    fn token_spans_contiguous(input in "[\\x00-\\x7F]{0,200}") {
        let result = parse(&input);
        let tokens = collect_tokens(&result.tree);

        if tokens.is_empty() {
            prop_assert_eq!(input.len(), 0, "No tokens but input is non-empty");
            return Ok(());
        }

        // First token starts at 0
        prop_assert_eq!(
            tokens[0].0, 0,
            "First token should start at 0, got {}",
            tokens[0].0
        );

        // Last token ends at source length
        let last = tokens.last().unwrap();
        prop_assert_eq!(
            last.1 as usize,
            input.len(),
            "Last token should end at source length {}, got {}",
            input.len(),
            last.1
        );

        // Each token's end == next token's start (contiguous, no gaps/overlaps)
        for window in tokens.windows(2) {
            prop_assert_eq!(
                window[0].1,
                window[1].0,
                "Gap or overlap between tokens: ({}, {}) and ({}, {})",
                window[0].0,
                window[0].1,
                window[1].0,
                window[1].1
            );
        }

        // Each token has start <= end
        for (start, end) in &tokens {
            prop_assert!(
                start <= end,
                "Token has start {} > end {}",
                start,
                end
            );
        }
    }

    /// Tree node ranges are consistent: start <= end, children within parent.
    #[test]
    fn tree_ranges_consistent(input in "[\\x00-\\x7F]{0,200}") {
        let result = parse(&input);
        assert_ranges_consistent(&result.tree);
    }
}
