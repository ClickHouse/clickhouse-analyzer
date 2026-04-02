use crate::parser::grammar::expressions::parse_expression;
use crate::parser::grammar::types::parse_column_type;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// True if the parser is at a query parameter: `{name:Type}`.
pub fn at_query_parameter(p: &mut Parser) -> bool {
    p.at(SyntaxKind::OpeningCurlyBrace)
        && p.nth(1) == SyntaxKind::BareWord
        && p.nth(2) == SyntaxKind::Colon
}

/// Parse a query parameter `{name:Type}` into a QueryParameterExpression node.
/// Caller must verify `at_query_parameter(p)` first.
pub fn parse_query_parameter(p: &mut Parser) {
    let m = p.start();
    p.expect(SyntaxKind::OpeningCurlyBrace);
    p.advance(); // parameter name
    p.expect(SyntaxKind::Colon);
    parse_column_type(p);
    p.expect(SyntaxKind::ClosingCurlyBrace);
    p.complete(m, SyntaxKind::QueryParameterExpression);
}

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
        if p.at_any(&[SyntaxKind::BareWord, SyntaxKind::QuotedIdentifier, SyntaxKind::StringToken])
        {
            p.advance();
        } else {
            p.advance_with_error("Expected cluster name after ON CLUSTER");
        }
        p.complete(m, SyntaxKind::OnClusterClause);
    }
}

/// Parse [db.]name, wrapping in TableIdentifier.
/// Each name slot can be a bare identifier, quoted identifier, or query parameter.
pub fn parse_table_identifier(p: &mut Parser) {
    let m = p.start();
    if p.at_identifier() || at_query_parameter(p) {
        parse_identifier_or_param(p);
        if p.at(SyntaxKind::Dot) {
            p.advance();
            if p.at_identifier() || at_query_parameter(p) {
                parse_identifier_or_param(p);
            } else {
                p.advance_with_error("Expected name after dot");
            }
        }
    } else {
        p.recover_with_error("Expected table name");
    }
    p.complete(m, SyntaxKind::TableIdentifier);
}

/// Parse either a bare/quoted identifier or a query parameter `{name:Type}`.
fn parse_identifier_or_param(p: &mut Parser) {
    if at_query_parameter(p) {
        parse_query_parameter(p);
    } else {
        p.advance(); // bare word or quoted identifier
    }
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

    p.expect(SyntaxKind::Equals);
    parse_expression(p);

    p.complete(m, SyntaxKind::SettingItem);
}

/// Parse optional SETTINGS clause: `SETTINGS key = value [, key = value, ...]`
/// Returns true if a SETTINGS clause was parsed.
pub fn parse_optional_settings_clause(p: &mut Parser) -> bool {
    if !p.at_keyword(Keyword::Settings) {
        return false;
    }

    let m = p.start();
    p.expect_keyword(Keyword::Settings);

    let mut first = true;
    while !p.eof() && !p.end_of_statement() {
        if !first {
            if !p.at(SyntaxKind::Comma) {
                break;
            }
            p.advance(); // comma
        }
        first = false;

        if !p.at_identifier() {
            break;
        }

        parse_setting_item(p);
    }

    p.complete(m, SyntaxKind::SettingsClause);
    true
}
