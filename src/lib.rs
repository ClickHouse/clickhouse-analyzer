mod diagnostics;
mod formatter;
mod lexer;
mod parser;

pub use diagnostics::{enrich_diagnostics, Diagnostic, RelatedSpan, Severity, Suggestion};
pub use formatter::{format, FormatConfig};
pub use lexer::token::Token;
pub use parser::diagnostic::{Parse, SyntaxError};
pub use parser::parse;
pub use parser::syntax_kind::SyntaxKind;
pub use parser::syntax_tree::{SyntaxChild, SyntaxTree};

#[cfg(feature = "wasm")]
mod wasm {
    use super::*;
    use std::panic;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn main() -> Result<(), JsValue> {
        panic::set_hook(Box::new(console_error_panic_hook::hook));
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_tree(sql: &str) -> String {
        let result = parse(sql);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        buf
    }

    #[wasm_bindgen]
    pub fn format_sql(sql: &str) -> String {
        let result = parse(sql);
        format(&result.tree, &FormatConfig::default(), &result.source)
    }

    #[wasm_bindgen]
    pub fn get_diagnostics(sql: &str) -> String {
        let result = parse(sql);
        let diagnostics = diagnostics::enrich_diagnostics(&result, sql);
        serde_json::to_string(&diagnostics).unwrap_or_default()
    }
}
