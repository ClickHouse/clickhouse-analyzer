mod formatter;
mod lexer;
mod parser;

pub use formatter::{format, FormatConfig};
pub use lexer::token::Token;
pub use parser::diagnostic::{Parse, SyntaxError};
pub use parser::parse;
pub use parser::syntax_kind::SyntaxKind;
pub use parser::syntax_tree::{SyntaxChild, SyntaxTree};

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
    result.tree.print(&mut buf, 0);
    buf
}

#[wasm_bindgen]
pub fn format_sql(sql: &str) -> String {
    let result = parse(sql);
    format(&result.tree, &FormatConfig::default())
}
