pub mod diagnostic;
pub(crate) mod event;
pub(crate) mod grammar;
pub(crate) mod interval_unit;
pub(crate) mod keyword;
pub(crate) mod marker;
#[allow(clippy::module_inception)]
pub(crate) mod parser;
pub mod syntax_kind;
pub mod syntax_tree;
pub(crate) mod token_set;

use crate::lexer::tokenizer::tokenize_with_whitespace;
use crate::parser::diagnostic::Parse;

pub fn parse(text: &str) -> Parse {
    let tokens = tokenize_with_whitespace(text);
    let mut p = parser::Parser::new(tokens);
    grammar::parse_source(&mut p);
    p.build_tree()
}
