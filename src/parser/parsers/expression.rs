use crate::lexer::token::TokenKind;
use crate::parser::keyword::Keyword;
use crate::parser::parser::{MarkClosed, Parser};
use crate::parser::parsers::column_type::parse_column_type;
use crate::parser::parsers::select::{
    at_end_of_column_list, at_select_statement, parse_select_statement,
};
use crate::parser::tree::TreeKind;

pub fn parse_expression(p: &mut Parser) {
    parse_expression_rec(p, TokenKind::EndOfStream);
}

pub fn parse_expression_rec(p: &mut Parser, left: TokenKind) {
    let Some(mut lhs) = expr_delimited(p) else {
        p.advance_with_error("Expected expression");
        return;
    };

    while p.at(TokenKind::OpeningRoundBracket) {
        let m = p.open_before(lhs);
        arg_list(p);
        lhs = p.close(m, TreeKind::FunctionCall);
    }

    loop {
        let mut right = p.nth(0);

        // Temporary hack for keyword operators
        if p.at_keyword(Keyword::And) {
            right = TokenKind::And;
        } else if p.at_keyword(Keyword::Or) {
            right = TokenKind::Or
        }

        if right_binds_tighter(left, right) {
            let m = p.open_before(lhs);
            p.advance();
            parse_expression_rec(p, right);
            lhs = p.close(m, TreeKind::BinaryExpression);
        } else {
            break;
        }
    }
}

fn right_binds_tighter(left: TokenKind, right: TokenKind) -> bool {
    fn tightness(kind: TokenKind) -> Option<usize> {
        [
            // Precedence table:
            &[TokenKind::And, TokenKind::Or],
            &[TokenKind::GreaterOrEquals, TokenKind::LessOrEquals],
            &[TokenKind::Equals, TokenKind::NotEquals],
            &[TokenKind::Greater, TokenKind::Less],
            &[TokenKind::Plus, TokenKind::Minus],
            &[TokenKind::Asterisk, TokenKind::Slash],
        ]
        .iter()
        .position(|level| level.contains(&kind))
    }
    let Some(right_tightness) = tightness(right) else {
        return false;
    };
    let Some(left_tightness) = tightness(left) else {
        assert_eq!(left, TokenKind::EndOfStream);
        return true;
    };
    right_tightness > left_tightness
}

fn expr_delimited(p: &mut Parser) -> Option<MarkClosed> {
    let result = match p.nth(0) {
        TokenKind::Asterisk => {
            let m = p.open();
            p.advance();
            p.close(m, TreeKind::Asterisk)
        }
        TokenKind::StringLiteral => {
            let m = p.open();
            p.advance();
            p.close(m, TreeKind::StringLiteral)
        }
        TokenKind::Number => {
            let m = p.open();
            p.advance();
            p.close(m, TreeKind::NumberLiteral)
        }
        TokenKind::BareWord | TokenKind::QuotedIdentifier => {
            let m = p.open();
            if at_select_statement(p) {
                parse_select_statement(p);
                p.close(m, TreeKind::SubqueryExpression)
            } else if !at_end_of_column_list(p) {
                p.advance();
                while p.at(TokenKind::Dot) && !p.eof() {
                    p.advance();
                    expr_delimited(p);
                }
                p.close(m, TreeKind::ColumnReference)
            } else {
                p.recover_with_error("Expected column identifier");
                p.close(m, TreeKind::ColumnReference)
            }
        }
        TokenKind::OpeningRoundBracket => {
            let m = p.open();
            p.expect(TokenKind::OpeningRoundBracket);
            parse_expression(p);
            let mut i = 0;
            while p.at(TokenKind::Comma) && !p.eof() {
                p.advance();
                parse_expression(p);
                i += 1;
            }

            p.expect(TokenKind::ClosingRoundBracket);
            if i > 0 {
                p.close(m, TreeKind::TupleExpression)
            } else {
                p.close(m, TreeKind::Expression)
            }
        }
        TokenKind::OpeningSquareBracket => {
            let m = p.open();
            p.expect(TokenKind::OpeningSquareBracket);

            parse_expression(p);

            while p.at(TokenKind::Comma) && !p.eof() {
                p.advance();
                parse_expression(p);
            }

            p.expect(TokenKind::ClosingSquareBracket);
            p.close(m, TreeKind::ArrayExpression)
        }
        _ => return None,
    };

    if p.at(TokenKind::DoubleColon) {
        let m = p.open_before(result);
        p.expect(TokenKind::DoubleColon);
        parse_column_type(p);
        return Some(p.close(m, TreeKind::CastExpression));
    }

    Some(result)
}

fn arg_list(p: &mut Parser) {
    let m = p.open();

    let mut first = true;
    p.expect(TokenKind::OpeningRoundBracket);
    while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
        if !first {
            p.expect(TokenKind::Comma);
        }
        // if p.at_any(EXPR_FIRST) {
        arg(p);
        first = false;
        // } else {
        //     break;
        // }
    }
    p.expect(TokenKind::ClosingRoundBracket);

    p.close(m, TreeKind::ExpressionList);
}

fn arg(p: &mut Parser) {
    let m = p.open();
    parse_expression(p);

    if p.at(TokenKind::Arrow) {
        p.advance();
        parse_expression(p);
        p.close(m, TreeKind::LambdaExpression);
        return;
    }

    p.close(m, TreeKind::Expression);
}
