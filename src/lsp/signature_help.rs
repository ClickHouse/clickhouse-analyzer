use tower_lsp::lsp_types::*;

use crate::analysis::cursor_context::{cursor_context, CursorContext};
use crate::metadata::cache::SharedMetadata;
use crate::parser::diagnostic::Parse;

use super::line_index::LineIndex;

pub async fn handle_signature_help(
    parse: &Parse,
    line_index: &LineIndex,
    position: Position,
    metadata: &SharedMetadata,
) -> Option<SignatureHelp> {
    let offset = line_index.offset(position);
    let ctx = cursor_context(&parse.tree, &parse.source, offset);

    let (function_name, argument_index) = match ctx {
        CursorContext::FunctionArgument {
            function_name,
            argument_index,
        } => (function_name, argument_index),
        _ => return None,
    };

    let meta = metadata.read().await;
    let info = meta.lookup_function(&function_name)?;

    // Parse parameter names from the syntax field.
    // Example: "toDateTime(expr[, timezone])" → ["expr", "timezone"]
    let params = parse_function_params(&info.syntax);

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: if info.syntax.is_empty() {
                format!("{}()", info.name)
            } else {
                info.syntax.clone()
            },
            documentation: if info.description.is_empty() {
                None
            } else {
                Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info.description.clone(),
                }))
            },
            parameters: Some(
                params
                    .iter()
                    .map(|p| ParameterInformation {
                        label: ParameterLabel::Simple(p.clone()),
                        documentation: None,
                    })
                    .collect(),
            ),
            active_parameter: Some(argument_index as u32),
        }],
        active_signature: Some(0),
        active_parameter: Some(argument_index as u32),
    })
}

/// Parse parameter names from a ClickHouse function syntax string.
/// Examples:
///   "toDateTime(expr[, timezone])" → ["expr", "timezone"]
///   "arrayJoin(arr)" → ["arr"]
///   "count()" → []
fn parse_function_params(syntax: &str) -> Vec<String> {
    // Find content between first '(' and last ')'
    let start = match syntax.find('(') {
        Some(i) => i + 1,
        None => return Vec::new(),
    };
    let end = match syntax.rfind(')') {
        Some(i) => i,
        None => return Vec::new(),
    };
    if start >= end {
        return Vec::new();
    }

    let inner = &syntax[start..end];
    if inner.trim().is_empty() {
        return Vec::new();
    }

    // Strip all square brackets (optional param notation), then split by comma
    let cleaned: String = inner.chars().filter(|&c| c != '[' && c != ']').collect();
    cleaned
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_params_basic() {
        assert_eq!(
            parse_function_params("toDateTime(expr[, timezone])"),
            vec!["expr", "timezone"]
        );
    }

    #[test]
    fn parse_params_single() {
        assert_eq!(
            parse_function_params("arrayJoin(arr)"),
            vec!["arr"]
        );
    }

    #[test]
    fn parse_params_empty() {
        let result: Vec<String> = Vec::new();
        assert_eq!(parse_function_params("count()"), result);
    }

    #[test]
    fn parse_params_no_parens() {
        let result: Vec<String> = Vec::new();
        assert_eq!(parse_function_params("something"), result);
    }
}
