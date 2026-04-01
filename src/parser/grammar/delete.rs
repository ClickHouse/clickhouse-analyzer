use crate::lexer::token::TokenKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Check if the parser is at a DELETE statement.
pub fn at_delete_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Delete)
}

/// Parse a DELETE statement.
///
/// ```text
/// DELETE FROM [db.]table [ON CLUSTER cluster] WHERE expr
/// ```
///
/// ClickHouse requires a WHERE clause for lightweight deletes.
pub fn parse_delete_statement(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Delete);

    // FROM is required
    if !p.eat_keyword(Keyword::From) {
        p.recover_with_error("Expected FROM after DELETE");
    }

    // Parse table identifier: [db.]table
    parse_table_identifier(p);

    // Optional: ON CLUSTER cluster_name
    if p.at_keyword(Keyword::On) {
        parse_on_cluster_clause(p);
    }

    // WHERE is required for ClickHouse lightweight deletes
    if p.at_keyword(Keyword::Where) {
        let mw = p.start();
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
        p.complete(mw, SyntaxKind::WhereClause);
    } else {
        p.recover_with_error("Expected WHERE clause in DELETE statement");
    }

    p.complete(m, SyntaxKind::DeleteStatement);
}

/// Parse a table identifier: [db.]table
fn parse_table_identifier(p: &mut Parser) {
    let m = p.start();

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();

        // Handle optional database.table notation
        if p.at(TokenKind::Dot) {
            p.advance();

            if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
                p.advance();
            } else {
                p.advance_with_error("Expected table name after dot");
            }
        }
    } else {
        p.advance_with_error("Expected table name");
    }

    p.complete(m, SyntaxKind::TableIdentifier);
}

/// Parse ON CLUSTER clause.
fn parse_on_cluster_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::On);
    p.expect_keyword(Keyword::Cluster);

    // Cluster name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier, TokenKind::StringLiteral]) {
        p.advance();
    } else {
        p.advance_with_error("Expected cluster name");
    }

    p.complete(m, SyntaxKind::OnClusterClause);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    #[test]
    fn test_delete_basic() {
        let sql = "DELETE FROM my_table WHERE id = 1";
        let result = parse(sql);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DeleteStatement"));
        assert!(buf.contains("TableIdentifier"));
        assert!(buf.contains("WhereClause"));
    }

    #[test]
    fn test_delete_with_db_table() {
        let sql = "DELETE FROM my_db.my_table WHERE id > 100";
        let result = parse(sql);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DeleteStatement"));
        assert!(buf.contains("TableIdentifier"));
    }

    #[test]
    fn test_delete_on_cluster() {
        let sql = "DELETE FROM my_table ON CLUSTER my_cluster WHERE status = 'inactive'";
        let result = parse(sql);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DeleteStatement"));
        assert!(buf.contains("OnClusterClause"));
        assert!(buf.contains("WhereClause"));
    }

    #[test]
    fn test_delete_missing_where() {
        let sql = "DELETE FROM my_table";
        let result = parse(sql);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DeleteStatement"));
        assert!(buf.contains("Error"));
    }

    #[test]
    fn test_delete_missing_from() {
        let sql = "DELETE my_table WHERE id = 1";
        let result = parse(sql);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DeleteStatement"));
        assert!(buf.contains("Error"));
    }
}
