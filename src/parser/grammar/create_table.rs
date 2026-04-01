use crate::lexer::token::TokenKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::grammar::select::{at_select_statement, parse_select_statement};
use crate::parser::grammar::types::parse_column_type;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

/// Check if the current position starts a CREATE statement.
pub fn at_create_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Create)
}

/// Parse a CREATE statement, dispatching to the appropriate sub-parser.
pub fn parse_create_statement(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Create);

    // OR REPLACE — just consume the tokens inline, no wrapping node
    if p.at_keyword(Keyword::Or) {
        p.advance(); // OR
        p.expect_keyword(Keyword::Replace);
    }

    // TEMPORARY
    let is_temporary = p.eat_keyword(Keyword::Temporary);

    if p.at_keyword(Keyword::Table) {
        parse_create_table(p);
    } else if p.at_keyword(Keyword::Database) {
        parse_create_database(p);
    } else if p.at_keyword(Keyword::Materialized) {
        parse_create_materialized_view(p);
    } else if p.at_keyword(Keyword::View) {
        parse_create_view(p);
    } else if p.at_keyword(Keyword::Function) {
        parse_create_function(p);
    } else if p.at_keyword(Keyword::Dictionary) {
        parse_create_dictionary(p);
    } else {
        if is_temporary {
            // TEMPORARY only valid with TABLE
            p.recover_with_error("Expected TABLE after TEMPORARY");
        } else {
            p.advance_with_error("Expected TABLE, DATABASE, VIEW, MATERIALIZED VIEW, FUNCTION, or DICTIONARY");
        }
    }

    p.complete(m, SyntaxKind::CreateStatement);
}

/// Parse CREATE TABLE ...
fn parse_create_table(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Table);

    // IF NOT EXISTS
    parse_if_not_exists(p);

    // Table name: [db.]table
    parse_table_identifier(p);

    // ON CLUSTER
    parse_on_cluster(p);

    // UUID (optional, skip string literal)
    if p.at_keyword(Keyword::As) && !p.at(TokenKind::OpeningRoundBracket) {
        // Could be CREATE TABLE ... AS other_table or CREATE TABLE ... AS SELECT
        // Check if next token after AS is a SELECT keyword
        parse_as_clause(p);
        // After AS clause, optionally parse ENGINE
        if p.at_keyword(Keyword::Engine) {
            parse_engine_clause(p);
        }
        p.complete(m, SyntaxKind::TableDefinition);
        return;
    }

    // Column definition list
    if p.at(TokenKind::OpeningRoundBracket) {
        parse_column_definition_list(p);
    }

    // ENGINE = ...
    if p.at_keyword(Keyword::Engine) {
        parse_engine_clause(p);
    }

    // Table-level clauses (can appear in any order after ENGINE)
    parse_table_clauses(p);

    // AS SELECT ... (at the end)
    if p.at_keyword(Keyword::As) {
        parse_as_clause(p);
    }

    p.complete(m, SyntaxKind::TableDefinition);
}

/// Parse CREATE DATABASE ...
fn parse_create_database(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Database);

    // IF NOT EXISTS
    parse_if_not_exists(p);

    // Database name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected database name");
    }

    // ON CLUSTER
    parse_on_cluster(p);

    // ENGINE = ...
    if p.at_keyword(Keyword::Engine) {
        parse_engine_clause(p);
    }

    // COMMENT
    if p.at_keyword(Keyword::Comment) {
        let m2 = p.start();
        p.advance(); // COMMENT
        if p.at(TokenKind::StringLiteral) {
            p.advance();
        } else {
            p.recover_with_error("Expected string literal after COMMENT");
        }
        p.complete(m2, SyntaxKind::ColumnComment);
    }

    p.complete(m, SyntaxKind::DatabaseDefinition);
}

/// Parse CREATE VIEW ...
fn parse_create_view(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::View);

    // IF NOT EXISTS
    parse_if_not_exists(p);

    // View name: [db.]view
    parse_table_identifier(p);

    // ON CLUSTER
    parse_on_cluster(p);

    // AS SELECT
    if p.at_keyword(Keyword::As) {
        parse_as_clause(p);
    } else {
        p.recover_with_error("Expected AS SELECT for VIEW definition");
    }

    p.complete(m, SyntaxKind::ViewDefinition);
}

/// Parse CREATE MATERIALIZED VIEW ...
fn parse_create_materialized_view(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Materialized);
    p.expect_keyword(Keyword::View);

    // IF NOT EXISTS
    parse_if_not_exists(p);

    // View name: [db.]view
    parse_table_identifier(p);

    // ON CLUSTER
    parse_on_cluster(p);

    // TO [db.]table
    if p.at_keyword(Keyword::To) {
        p.advance(); // TO
        parse_table_identifier(p);
    }

    // Column definition list (optional)
    if p.at(TokenKind::OpeningRoundBracket) {
        parse_column_definition_list(p);
    }

    // ENGINE = ...
    if p.at_keyword(Keyword::Engine) {
        parse_engine_clause(p);
    }

    // Table-level clauses
    parse_table_clauses(p);

    // POPULATE
    p.eat_keyword(Keyword::Populate);

    // AS SELECT
    if p.at_keyword(Keyword::As) {
        parse_as_clause(p);
    } else {
        p.recover_with_error("Expected AS SELECT for MATERIALIZED VIEW definition");
    }

    p.complete(m, SyntaxKind::MaterializedViewDefinition);
}

/// Parse CREATE FUNCTION ...
fn parse_create_function(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Function);

    // Function name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected function name");
    }

    // AS (args) -> expr  or  AS expr
    p.expect_keyword(Keyword::As);

    // Parse the lambda body: expression, then check for ->
    let lm = p.start();
    parse_expression(p);
    if p.at(TokenKind::Arrow) {
        p.advance(); // ->
        parse_expression(p);
        p.complete(lm, SyntaxKind::LambdaExpression);
    } else {
        p.complete(lm, SyntaxKind::Expression);
    }

    p.complete(m, SyntaxKind::FunctionDefinition);
}

/// Parse CREATE DICTIONARY ...
fn parse_create_dictionary(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Dictionary);

    // IF NOT EXISTS
    parse_if_not_exists(p);

    // Dictionary name: [db.]dict
    parse_table_identifier(p);

    // ON CLUSTER
    parse_on_cluster(p);

    // Column definition list
    if p.at(TokenKind::OpeningRoundBracket) {
        parse_column_definition_list(p);
    }

    // PRIMARY KEY
    if p.at_keyword(Keyword::Primary) {
        let m2 = p.start();
        p.advance(); // PRIMARY
        p.expect_keyword(Keyword::Key);
        parse_expression(p);
        p.complete(m2, SyntaxKind::PrimaryKeyDefinition);
    }

    // SOURCE(...)
    if p.at_keyword(Keyword::Source) {
        parse_parenthesized_clause(p);
    }

    // LAYOUT(...)
    if p.at_keyword(Keyword::Layout) {
        parse_parenthesized_clause(p);
    }

    // LIFETIME(...)
    if p.at_keyword(Keyword::Lifetime) {
        parse_parenthesized_clause(p);
    }

    p.complete(m, SyntaxKind::DictionaryDefinition);
}

/// Parse a parenthesized clause like SOURCE(...), LAYOUT(...), LIFETIME(...)
fn parse_parenthesized_clause(p: &mut Parser) {
    let m = p.start();
    p.advance(); // keyword (SOURCE, LAYOUT, LIFETIME)
    if p.at(TokenKind::OpeningRoundBracket) {
        p.advance(); // (
        parse_parenthesized_content(p);
        p.expect(TokenKind::ClosingRoundBracket);
    }
    p.complete(m, SyntaxKind::Expression);
}

/// Parse content inside parentheses, handling nested parens.
fn parse_parenthesized_content(p: &mut Parser) {
    while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
        if p.at(TokenKind::OpeningRoundBracket) {
            p.advance(); // (
            parse_parenthesized_content(p);
            p.expect(TokenKind::ClosingRoundBracket);
        } else {
            p.advance();
        }
    }
}

/// Parse IF NOT EXISTS
fn parse_if_not_exists(p: &mut Parser) {
    if p.at_keyword(Keyword::If) {
        let m = p.start();
        p.advance(); // IF
        p.expect_keyword(Keyword::Not);
        p.expect_keyword(Keyword::Exists);
        p.complete(m, SyntaxKind::IfNotExistsClause);
    }
}

/// Parse ON CLUSTER cluster_name
fn parse_on_cluster(p: &mut Parser) {
    if p.at_keyword(Keyword::On) {
        let m = p.start();
        p.advance(); // ON
        p.expect_keyword(Keyword::Cluster);
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier, TokenKind::StringLiteral])
        {
            p.advance();
        } else {
            p.recover_with_error("Expected cluster name after ON CLUSTER");
        }
        p.complete(m, SyntaxKind::OnClusterClause);
    }
}

/// Parse a table identifier: [db.]table
fn parse_table_identifier(p: &mut Parser) {
    let m = p.start();

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();

        // Handle db.table
        if p.at(TokenKind::Dot) {
            p.advance(); // .
            if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
                p.advance();
            } else {
                p.advance_with_error("Expected table name after dot");
            }
        }
    } else {
        p.recover_with_error("Expected table name");
    }

    p.complete(m, SyntaxKind::TableIdentifier);
}

/// Parse AS clause: AS SELECT ... or AS [db.]table
fn parse_as_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::As);

    if at_select_statement(p) {
        parse_select_statement(p);
    } else if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        parse_table_identifier(p);
    } else {
        p.recover_with_error("Expected SELECT or table name after AS");
    }

    p.complete(m, SyntaxKind::AsClause);
}

/// Parse the parenthesized column definition list.
pub fn parse_column_definition_list(p: &mut Parser) {
    let m = p.start();

    p.expect(TokenKind::OpeningRoundBracket);

    let mut first = true;
    while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
        if !first {
            p.expect(TokenKind::Comma);
        }
        first = false;

        if p.at(TokenKind::ClosingRoundBracket) {
            break;
        }

        // Decide what kind of definition this is
        if p.at_keyword(Keyword::Index) {
            parse_index_definition(p);
        } else if p.at_keyword(Keyword::Projection) {
            parse_projection_definition(p);
        } else if p.at_keyword(Keyword::Constraint) {
            parse_constraint_definition(p);
        } else {
            parse_column_definition(p);
        }
    }

    p.expect(TokenKind::ClosingRoundBracket);

    p.complete(m, SyntaxKind::ColumnDefinitionList);
}

/// Parse a single column definition: name Type [DEFAULT|MATERIALIZED|ALIAS|EPHEMERAL expr] [CODEC(codec)] [TTL expr] [COMMENT 'comment']
fn parse_column_definition(p: &mut Parser) {
    let m = p.start();

    // Column name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.advance_with_error("Expected column name");
        p.complete(m, SyntaxKind::ColumnDefinition);
        return;
    }

    // Column type (required in most cases, but not if DEFAULT etc. follows directly)
    // Parse type if next token looks like a type name (not a keyword like DEFAULT, CODEC, etc.)
    if at_column_type(p) {
        parse_column_type(p);
    }

    // DEFAULT | MATERIALIZED | ALIAS | EPHEMERAL
    if p.at_keyword(Keyword::Default)
        || p.at_keyword(Keyword::Materialized)
        || p.at_keyword(Keyword::Alias)
        || p.at_keyword(Keyword::Ephemeral)
    {
        let m2 = p.start();
        p.advance(); // the keyword
        // Expression (optional for EPHEMERAL)
        if !at_column_constraint_start(p)
            && !p.at(TokenKind::Comma)
            && !p.at(TokenKind::ClosingRoundBracket)
            && !p.eof()
        {
            parse_expression(p);
        }
        p.complete(m2, SyntaxKind::ColumnDefault);
    }

    // CODEC(...)
    if p.at_keyword(Keyword::Codec) {
        let m2 = p.start();
        p.advance(); // CODEC
        if p.at(TokenKind::OpeningRoundBracket) {
            p.advance();
            parse_codec_args(p);
            p.expect(TokenKind::ClosingRoundBracket);
        }
        p.complete(m2, SyntaxKind::ColumnCodec);
    }

    // TTL expr
    if p.at_keyword(Keyword::Ttl) {
        let m2 = p.start();
        p.advance(); // TTL
        parse_expression(p);
        p.complete(m2, SyntaxKind::ColumnTtl);
    }

    // COMMENT 'string'
    if p.at_keyword(Keyword::Comment) {
        let m2 = p.start();
        p.advance(); // COMMENT
        if p.at(TokenKind::StringLiteral) {
            p.advance();
        } else {
            p.recover_with_error("Expected string literal after COMMENT");
        }
        p.complete(m2, SyntaxKind::ColumnComment);
    }

    p.complete(m, SyntaxKind::ColumnDefinition);
}

/// Check if current pos looks like it could be a column type
fn at_column_type(p: &mut Parser) -> bool {
    if !p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        return false;
    }
    // Not a column type if it's a known constraint keyword
    if p.at_keyword(Keyword::Default)
        || p.at_keyword(Keyword::Materialized)
        || p.at_keyword(Keyword::Alias)
        || p.at_keyword(Keyword::Ephemeral)
        || p.at_keyword(Keyword::Codec)
        || p.at_keyword(Keyword::Ttl)
        || p.at_keyword(Keyword::Comment)
    {
        return false;
    }
    true
}

/// Check if at start of a column-level constraint keyword
fn at_column_constraint_start(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Codec)
        || p.at_keyword(Keyword::Ttl)
        || p.at_keyword(Keyword::Comment)
}

/// Parse CODEC arguments (comma-separated identifiers with optional params)
fn parse_codec_args(p: &mut Parser) {
    while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
            // Optional parameters
            if p.at(TokenKind::OpeningRoundBracket) {
                p.advance();
                // Parse codec parameters
                let mut first = true;
                while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
                    if !first {
                        p.expect(TokenKind::Comma);
                    }
                    first = false;
                    if !p.at(TokenKind::ClosingRoundBracket) {
                        parse_expression(p);
                    }
                }
                p.expect(TokenKind::ClosingRoundBracket);
            }
        } else if p.at(TokenKind::Comma) {
            p.advance();
        } else {
            p.advance_with_error("Expected codec name");
        }
    }
}

/// Parse INDEX definition: INDEX name expr TYPE type GRANULARITY val
fn parse_index_definition(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Index);

    // Index name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected index name");
    }

    // Expression
    parse_expression(p);

    // TYPE type_name
    if p.at_keyword(Keyword::Type) {
        p.advance(); // TYPE
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
            // Optional parameters
            if p.at(TokenKind::OpeningRoundBracket) {
                p.advance();
                let mut first = true;
                while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
                    if !first {
                        p.expect(TokenKind::Comma);
                    }
                    first = false;
                    parse_expression(p);
                }
                p.expect(TokenKind::ClosingRoundBracket);
            }
        } else {
            p.recover_with_error("Expected index type name");
        }
    }

    // GRANULARITY val
    if p.at_keyword(Keyword::Granularity) {
        p.advance(); // GRANULARITY
        parse_expression(p);
    }

    p.complete(m, SyntaxKind::IndexDefinition);
}

/// Parse PROJECTION definition: PROJECTION name (SELECT ...)
fn parse_projection_definition(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Projection);

    // Projection name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected projection name");
    }

    // (SELECT ...)
    if p.at(TokenKind::OpeningRoundBracket) {
        p.advance(); // (
        if at_select_statement(p) {
            parse_select_statement(p);
        } else {
            p.recover_with_error("Expected SELECT inside PROJECTION");
        }
        p.expect(TokenKind::ClosingRoundBracket);
    } else {
        p.recover_with_error("Expected ( after projection name");
    }

    p.complete(m, SyntaxKind::ProjectionDefinition);
}

/// Parse CONSTRAINT definition: CONSTRAINT name CHECK expr
fn parse_constraint_definition(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Constraint);

    // Constraint name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected constraint name");
    }

    // CHECK expr
    if p.at_keyword(Keyword::Check) {
        p.advance(); // CHECK
        parse_expression(p);
    } else {
        p.recover_with_error("Expected CHECK after constraint name");
    }

    p.complete(m, SyntaxKind::ConstraintDefinition);
}

/// Parse ENGINE = Name(args)
fn parse_engine_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Engine);
    p.expect(TokenKind::Equals);

    // Engine name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected engine name");
        p.complete(m, SyntaxKind::EngineClause);
        return;
    }

    // Optional arguments
    if p.at(TokenKind::OpeningRoundBracket) {
        p.advance(); // (
        let mut first = true;
        while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
            if !first {
                p.expect(TokenKind::Comma);
            }
            first = false;
            if !p.at(TokenKind::ClosingRoundBracket) {
                parse_expression(p);
            }
        }
        p.expect(TokenKind::ClosingRoundBracket);
    }

    p.complete(m, SyntaxKind::EngineClause);
}

/// Parse table-level clauses: ORDER BY, PARTITION BY, PRIMARY KEY, SAMPLE BY, TTL, SETTINGS, COMMENT
fn parse_table_clauses(p: &mut Parser) {
    loop {
        if p.at_keyword(Keyword::Order) {
            let m = p.start();
            p.advance(); // ORDER
            p.expect_keyword(Keyword::By);
            parse_order_by_expr(p);
            p.complete(m, SyntaxKind::OrderByDefinition);
        } else if p.at_keyword(Keyword::Partition) {
            let m = p.start();
            p.advance(); // PARTITION
            p.expect_keyword(Keyword::By);
            parse_expression(p);
            p.complete(m, SyntaxKind::PartitionByDefinition);
        } else if p.at_keyword(Keyword::Primary) {
            let m = p.start();
            p.advance(); // PRIMARY
            p.expect_keyword(Keyword::Key);
            parse_expression(p);
            p.complete(m, SyntaxKind::PrimaryKeyDefinition);
        } else if p.at_keyword(Keyword::Sample) {
            let m = p.start();
            p.advance(); // SAMPLE
            p.expect_keyword(Keyword::By);
            parse_expression(p);
            p.complete(m, SyntaxKind::SampleByDefinition);
        } else if p.at_keyword(Keyword::Ttl) {
            let m = p.start();
            p.advance(); // TTL
            parse_expression(p);
            p.complete(m, SyntaxKind::TtlDefinition);
        } else if p.at_keyword(Keyword::Settings) {
            parse_settings_clause(p);
        } else if p.at_keyword(Keyword::Comment) {
            let m = p.start();
            p.advance(); // COMMENT
            if p.at(TokenKind::StringLiteral) {
                p.advance();
            } else {
                p.recover_with_error("Expected string literal after COMMENT");
            }
            p.complete(m, SyntaxKind::ColumnComment);
        } else {
            break;
        }
    }
}

/// Parse ORDER BY expression - can be a single expression or a tuple
fn parse_order_by_expr(p: &mut Parser) {
    parse_expression(p);
}

/// Parse SETTINGS clause: SETTINGS key = value, ...
fn parse_settings_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Settings);

    let mut first = true;
    while !p.end_of_statement() && !p.eof() {
        // Stop if we hit another clause keyword
        if p.at_keyword(Keyword::Comment) || p.at_keyword(Keyword::As) {
            break;
        }

        if !first {
            if !p.eat(TokenKind::Comma) {
                break;
            }
        }
        first = false;

        let m2 = p.start();
        // key = value
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance(); // key
            p.expect(TokenKind::Equals);
            parse_expression(p);
        } else {
            p.advance_with_error("Expected setting name");
        }
        p.complete(m2, SyntaxKind::SettingItem);
    }

    p.complete(m, SyntaxKind::SettingsClause);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    #[test]
    fn test_create_table_basic() {
        let result = parse("CREATE TABLE test (id UInt64, name String) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        // Just verify it parses without panic and has CreateStatement
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
        assert!(buf.contains("ColumnDefinitionList"));
        assert!(buf.contains("ColumnDefinition"));
        assert!(buf.contains("EngineClause"));
        assert!(buf.contains("OrderByDefinition"));
    }

    #[test]
    fn test_create_table_if_not_exists() {
        let result = parse("CREATE TABLE IF NOT EXISTS db.test (id UInt64) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("TableIdentifier"));
    }

    #[test]
    fn test_create_table_on_cluster() {
        let result = parse("CREATE TABLE test ON CLUSTER my_cluster (id UInt64) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("OnClusterClause"));
    }

    #[test]
    fn test_create_table_column_defaults() {
        let result = parse("CREATE TABLE test (id UInt64 DEFAULT 0, name String ALIAS 'hello', ts DateTime CODEC(Delta, ZSTD)) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("ColumnDefault"));
        assert!(buf.contains("ColumnCodec"));
    }

    #[test]
    fn test_create_table_column_comment() {
        let result = parse("CREATE TABLE test (id UInt64 COMMENT 'primary key') ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("ColumnComment"));
    }

    #[test]
    fn test_create_table_index() {
        let result = parse("CREATE TABLE test (id UInt64, INDEX idx id TYPE minmax GRANULARITY 3) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("IndexDefinition"));
    }

    #[test]
    fn test_create_table_constraint() {
        let result = parse("CREATE TABLE test (id UInt64, CONSTRAINT c1 CHECK id > 0) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("ConstraintDefinition"));
    }

    #[test]
    fn test_create_table_as_select() {
        let result = parse("CREATE TABLE test AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("AsClause"));
        assert!(buf.contains("SelectStatement"));
    }

    #[test]
    fn test_create_table_as_other_table() {
        let result = parse("CREATE TABLE test AS other_table");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("AsClause"));
        assert!(buf.contains("TableIdentifier"));
    }

    #[test]
    fn test_create_table_with_engine_as_select() {
        let result = parse("CREATE TABLE test ENGINE = MergeTree() ORDER BY id AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("EngineClause"));
        assert!(buf.contains("AsClause"));
    }

    #[test]
    fn test_create_or_replace_table() {
        let result = parse("CREATE OR REPLACE TABLE test (id UInt64) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
    }

    #[test]
    fn test_create_temporary_table() {
        let result = parse("CREATE TEMPORARY TABLE test (id UInt64) ENGINE = Memory()");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
    }

    #[test]
    fn test_create_database() {
        let result = parse("CREATE DATABASE IF NOT EXISTS mydb ENGINE = Atomic() COMMENT 'test db'");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DatabaseDefinition"));
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("EngineClause"));
    }

    #[test]
    fn test_create_view() {
        let result = parse("CREATE VIEW myview AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("ViewDefinition"));
        assert!(buf.contains("AsClause"));
    }

    #[test]
    fn test_create_materialized_view() {
        let result = parse("CREATE MATERIALIZED VIEW myview TO dest_table AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("MaterializedViewDefinition"));
    }

    #[test]
    fn test_create_function() {
        let result = parse("CREATE FUNCTION myFunc AS (x) -> x + 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("FunctionDefinition"));
    }

    #[test]
    fn test_create_table_all_clauses() {
        let result = parse(
            "CREATE TABLE test (id UInt64) ENGINE = MergeTree() ORDER BY id PARTITION BY id PRIMARY KEY id SAMPLE BY id TTL id SETTINGS index_granularity = 8192 COMMENT 'test'"
        );
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("OrderByDefinition"));
        assert!(buf.contains("PartitionByDefinition"));
        assert!(buf.contains("PrimaryKeyDefinition"));
        assert!(buf.contains("SampleByDefinition"));
        assert!(buf.contains("TtlDefinition"));
        assert!(buf.contains("SettingsClause"));
        assert!(buf.contains("SettingItem"));
    }

    #[test]
    fn test_create_table_projection() {
        let result = parse("CREATE TABLE test (id UInt64, PROJECTION p1 (SELECT id ORDER BY id)) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("ProjectionDefinition"));
    }

    #[test]
    fn test_create_table_complex_types() {
        let result = parse("CREATE TABLE test (id UInt64, data Array(String), nested Tuple(a UInt32, b String)) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DataType"));
    }

    #[test]
    fn test_create_table_ttl_column() {
        let result = parse("CREATE TABLE test (id UInt64, ts DateTime TTL ts + 1) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("ColumnTtl"));
    }

    #[test]
    fn test_error_recovery_missing_engine() {
        // Should parse without panic even without ENGINE
        let result = parse("CREATE TABLE test (id UInt64)");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
    }

    #[test]
    fn test_error_recovery_missing_table_name() {
        // Should not panic
        let result = parse("CREATE TABLE");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("CreateStatement"));
    }

    #[test]
    fn test_create_dictionary() {
        let result = parse("CREATE DICTIONARY mydict (id UInt64, name String) PRIMARY KEY id SOURCE(CLICKHOUSE(host 'localhost')) LAYOUT(HASHED()) LIFETIME(300)");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        assert!(buf.contains("DictionaryDefinition"));
        assert!(buf.contains("PrimaryKeyDefinition"));
    }
}
