use crate::lexer::token::TokenKind;
use crate::parser::parser::Parser;
use crate::parser::tree::TreeKind;

pub fn parse_column_type(p: &mut Parser) {
    let m = p.open();

    if p.at(TokenKind::BareWord) {
        p.advance();
    } else {
        p.advance_with_error("Expected type for cast operator");
    }

    if p.at(TokenKind::OpeningRoundBracket) {
        let m = p.open();
        p.expect(TokenKind::OpeningRoundBracket);
        parse_column_type(p);

        while p.at(TokenKind::Comma) && !p.eof() {
            p.expect(TokenKind::Comma);
            parse_column_type(p);
        }

        p.expect(TokenKind::ClosingRoundBracket);
        p.close(m, TreeKind::DataTypeParameters);
    }

    p.close(m, TreeKind::DataType);
}
