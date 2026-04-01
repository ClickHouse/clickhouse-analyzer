pub mod expressions;
pub mod select;
pub mod types;

use crate::lexer::token::TokenKind;
use crate::parser::grammar::select::{at_select_statement, parse_select_statement};
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Top-level grammar entry point. Parses a full source file containing
/// one or more semicolon-separated SQL statements.
pub fn parse_source(p: &mut Parser) {
    let m = p.start();

    while !p.eof() {
        if at_select_statement(p) {
            parse_select_statement(p);
        } else if p.at(TokenKind::Semicolon) {
            p.advance();
        } else {
            p.advance_with_error("Unexpected token");
        }
    }

    p.skip_trivia();

    p.complete(m, SyntaxKind::File);
}
