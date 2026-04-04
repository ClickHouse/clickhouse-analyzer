/// A parse error: a message and the byte range in the source where it occurred.
///
/// This is the parser-level error type. It intentionally stores only byte offsets —
/// line/column resolution is the caller's responsibility (LSP, CLI, etc.) and is
/// trivially computed from the byte offset + source text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SyntaxError {
    pub message: String,
    pub range: (usize, usize),
}

/// Result of parsing: the syntax tree plus any errors collected.
#[derive(Clone)]
pub struct Parse {
    pub tree: super::syntax_tree::SyntaxTree,
    pub errors: Vec<SyntaxError>,
    pub source: String,
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    /// Format errors as `offset..offset: message` — stable, byte-based, no line/col coupling.
    fn check_errors(input: &str, expected: Expect) {
        let result = parse(input);
        let actual: String = result
            .errors
            .iter()
            .map(|e| format!("{}..{}: {}\n", e.range.0, e.range.1, e.message))
            .collect();
        expected.assert_eq(&actual);
    }

    #[test]
    fn no_errors_on_valid_input() {
        check_errors("SELECT 1", expect![[""]]);
    }

    #[test]
    fn missing_expression_after_select() {
        check_errors("SELECT", expect![""]);
    }

    #[test]
    fn missing_closing_paren() {
        check_errors("SELECT (1 + 2", expect![[r#"
            13..13: expected )
        "#]]);
    }

    #[test]
    fn bad_interval_unit() {
        check_errors("SELECT INTERVAL 5 POTATO", expect![[r#"
            18..24: Expected interval unit (e.g. SECOND, MINUTE, HOUR, DAY, WEEK, MONTH, QUARTER, YEAR)
        "#]]);
    }

    #[test]
    fn expected_table_after_from() {
        check_errors("SELECT 1 FROM", expect![[r#"
            13..13: Expected table reference
        "#]]);
    }

    #[test]
    fn expected_table_after_dot() {
        check_errors("SELECT 1 FROM db.", expect![[r#"
            17..17: Expected table name after dot
        "#]]);
    }

    #[test]
    fn multiple_errors() {
        check_errors("SELECT (1 FROM", expect![[r#"
            10..14: expected )
            14..14: Expected table reference
        "#]]);
    }
}
