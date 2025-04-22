use crate::lexer::token::TokenKind;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::parsers::expression::parse_expression;
use crate::parser::tree::TreeKind;

pub fn parse_select_statement(p: &mut Parser) {
    let m = p.open();

    if p.at_keyword(Keyword::With) {
        parse_with_clause(p);
    }

    let mut parsed_early_from = false;
    if p.at_keyword(Keyword::From) {
        parse_from_clause(p);
        parsed_early_from = true;
    }

    parse_select_clause(p);

    if p.at_keyword(Keyword::From) {
        parse_from_clause(p);

        if parsed_early_from {
            p.recover_with_error("Duplicate FROM clause");
        }
    }

    if p.at_keyword(Keyword::Where) {
        let m = p.open();
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
        p.close(m, TreeKind::WhereClause);
    }

    if p.at_keyword(Keyword::Order) {
        let m = p.open();
        p.expect_keyword(Keyword::Order);
        p.expect_keyword(Keyword::By);
        let m2 = p.open();
        parse_expression(p);
        p.close(m2, TreeKind::OrderByItem);
        p.close(m, TreeKind::OrderByClause);
    }

    if p.at_keyword(Keyword::Limit) {
        let m = p.open();
        p.expect_keyword(Keyword::Limit);
        parse_expression(p);
        p.close(m, TreeKind::LimitClause);
    }

    p.close(m, TreeKind::SelectStatement);
}

// Finds the end of a WITH or a SELECT
pub fn at_end_of_column_list(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Select)
        || p.at_keyword(Keyword::From)
        || p.at_keyword(Keyword::Where)
        || p.at_keyword(Keyword::Order)
        || p.at_keyword(Keyword::Limit)
}

pub fn at_select_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::With) || p.at_keyword(Keyword::Select) || p.at_keyword(Keyword::From)
}

pub fn parse_with_clause(p: &mut Parser) {
    let m = p.open();

    p.expect_keyword(Keyword::With);

    // Parse column list
    parse_column_list(p);

    p.close(m, TreeKind::WithClause);
}

pub fn parse_select_clause(p: &mut Parser) {
    let m = p.open();

    p.expect_keyword(Keyword::Select);

    // Parse column list
    parse_column_list(p);

    p.close(m, TreeKind::SelectClause);
}

// Parse a comma-separated list of column expressions
pub fn parse_column_list(p: &mut Parser) {
    let m = p.open();

    let mut first = true;
    while !at_end_of_column_list(p) && !p.end_of_statement() {
        if !first {
            p.expect(TokenKind::Comma);
        }
        first = false;

        parse_expression(p);

        if p.at_keyword(Keyword::As)
            || (!at_end_of_column_list(p) && p.at(TokenKind::BareWord))
            || p.at(TokenKind::QuotedIdentifier)
        {
            let m = p.open();
            if p.at_keyword(Keyword::As) {
                p.expect_keyword(Keyword::As);
            }

            if !at_end_of_column_list(p) {
                p.advance()
            } else {
                p.recover_with_error("Expected column alias");
            }

            p.close(m, TreeKind::ColumnAlias);
        }
    }

    p.close(m, TreeKind::ColumnList);
}

// Parse the FROM clause
fn parse_from_clause(p: &mut Parser) {
    let m = p.open();

    p.expect_keyword(Keyword::From);

    parse_table_reference(p);

    p.close(m, TreeKind::FromClause);
}

// Parse a table reference
fn parse_table_reference(p: &mut Parser) {
    let m = p.open();

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        // Simple table name
        p.advance();

        // Handle optional database.table notation
        if p.at(TokenKind::Dot) {
            p.advance(); // Consume dot

            if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
                p.advance();
            } else {
                p.advance_with_error("Expected table name after dot");
            }
        }
    } else {
        p.advance_with_error("Expected table reference");
    }

    p.close(m, TreeKind::TableIdentifier);
}
