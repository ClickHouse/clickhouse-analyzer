mod types;
mod bracket_matching;
mod context;

pub use types::{Diagnostic, Severity, Suggestion, RelatedSpan};

use crate::parser::diagnostic::Parse;

pub fn enrich_diagnostics(parse: &Parse, _source: &str) -> Vec<Diagnostic> {
    // Convert raw SyntaxErrors to base Diagnostics
    let mut diagnostics: Vec<Diagnostic> = parse.errors.iter().map(|e| {
        Diagnostic {
            message: e.message.clone(),
            range: e.range,
            severity: Severity::Error,
            code: None,
            suggestion: None,
            related: Vec::new(),
        }
    }).collect();

    // Run enrichment passes
    bracket_matching::enrich(&mut diagnostics, &parse.tree);
    context::enrich(&mut diagnostics, &parse.tree);

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    fn check_diagnostics(input: &str, expected: Expect) {
        let result = parse(input);
        let diagnostics = enrich_diagnostics(&result, input);
        let actual: String = diagnostics
            .iter()
            .map(|d| {
                let mut s = format!("{}..{}: [{}] {}", d.range.0, d.range.1,
                    match d.severity { Severity::Error => "error", Severity::Warning => "warning", Severity::Hint => "hint" },
                    d.message);
                if let Some(ref sug) = d.suggestion {
                    s += &format!(" (suggestion: {})", sug.message);
                }
                for r in &d.related {
                    s += &format!(" [related: {}..{}: {}]", r.range.0, r.range.1, r.message);
                }
                s += "\n";
                s
            })
            .collect();
        expected.assert_eq(&actual);
    }

    #[test]
    fn no_diagnostics_on_valid_sql() {
        check_diagnostics("SELECT 1 FROM t", expect![[""]]);
    }

    #[test]
    fn bracket_matching_related_span() {
        let result = parse("SELECT (1 + 2");
        let diagnostics = enrich_diagnostics(&result, "SELECT (1 + 2");
        let has_related = diagnostics.iter().any(|d| {
            d.related.iter().any(|r| r.message.contains("Unclosed bracket"))
        });
        assert!(has_related, "Expected related span for unclosed bracket, got: {:?}", diagnostics);
    }

    #[test]
    fn contextual_error_message() {
        // Use an input that produces "Unexpected token" within a recognizable clause
        let result = parse("SELECT 1 FROM t WHERE abc!def");
        let diagnostics = enrich_diagnostics(&result, "SELECT 1 FROM t WHERE abc!def");
        // Check that at least one diagnostic mentions a clause or statement context
        let has_context = diagnostics.iter().any(|d| {
            d.message.contains("WHERE")
                || d.message.contains("SELECT")
                || d.message.contains("statement")
                || d.message.contains("clause")
        });
        assert!(has_context, "Expected contextual message, got: {:?}", diagnostics);
    }
}
