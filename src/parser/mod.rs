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
use crate::parser::grammar::parse_source;
use crate::parser::syntax_tree::SyntaxTree;

pub fn parse(text: &str) -> SyntaxTree {
    let tokens = tokenize_with_whitespace(text);
    let mut p = parser::Parser::new(tokens);
    parse_source(&mut p);
    p.build_tree()
}
