use crate::lexer::token::TokenKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Parse optional IF EXISTS, wrapping in IfExistsClause.
pub fn parse_if_exists(p: &mut Parser) {
    if p.at_keyword(Keyword::If) {
        let m = p.start();
        p.expect_keyword(Keyword::If);
        p.expect_keyword(Keyword::Exists);
        p.complete(m, SyntaxKind::IfExistsClause);
    }
}

/// Parse optional IF NOT EXISTS, wrapping in IfNotExistsClause.
pub fn parse_if_not_exists(p: &mut Parser) {
    if p.at_keyword(Keyword::If) {
        let m = p.start();
        p.expect_keyword(Keyword::If);
        p.expect_keyword(Keyword::Not);
        p.expect_keyword(Keyword::Exists);
        p.complete(m, SyntaxKind::IfNotExistsClause);
    }
}

/// Parse optional ON CLUSTER name, wrapping in OnClusterClause.
pub fn parse_on_cluster(p: &mut Parser) {
    if p.at_keyword(Keyword::On) {
        let m = p.start();
        p.expect_keyword(Keyword::On);
        p.expect_keyword(Keyword::Cluster);
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier, TokenKind::StringLiteral])
        {
            p.advance();
        } else {
            p.advance_with_error("Expected cluster name after ON CLUSTER");
        }
        p.complete(m, SyntaxKind::OnClusterClause);
    }
}

/// Parse [db.]name, wrapping in TableIdentifier.
pub fn parse_table_identifier(p: &mut Parser) {
    let m = p.start();
    if p.at_identifier() {
        p.advance();
        if p.at(TokenKind::Dot) {
            p.advance();
            if p.at_identifier() {
                p.advance();
            } else {
                p.advance_with_error("Expected name after dot");
            }
        }
    } else {
        p.recover_with_error("Expected table name");
    }
    p.complete(m, SyntaxKind::TableIdentifier);
}

// ---------------------------------------------------------------------------
// Error recovery infrastructure
// ---------------------------------------------------------------------------

/// True if the current token exactly matches any keyword in the set.
pub fn at_any_keyword(p: &mut Parser, keywords: &[Keyword]) -> bool {
    keywords.iter().any(|kw| p.at_keyword(*kw))
}

/// Skip unexpected tokens until we reach a recognized keyword, end of statement,
/// or EOF. Each skipped token is wrapped in an Error node.
pub fn skip_to_keywords(p: &mut Parser, keywords: &[Keyword]) {
    while !p.eof() && !p.end_of_statement() && !at_any_keyword(p, keywords) {
        p.advance_with_error("Unexpected token");
    }
}

// ---------------------------------------------------------------------------
// Setting parsing
// ---------------------------------------------------------------------------

/// Parse a single setting: `key = value`.
pub fn parse_setting_item(p: &mut Parser) {
    let m = p.start();

    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected setting name");
    }

    p.expect(TokenKind::Equals);
    parse_expression(p);

    p.complete(m, SyntaxKind::SettingItem);
}
