mod lexer;
mod parser;

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
    let cst = parse(sql);
    let mut buf = String::new();
    cst.print(&mut buf, 0);
    buf
}
