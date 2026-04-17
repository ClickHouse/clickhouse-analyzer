use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::grammar::select::{at_select_statement, parse_select_statement};
use crate::parser::grammar::types::parse_column_type;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;

const CREATE_TABLE_KEYWORDS: &[Keyword] = &[
    Keyword::Engine, Keyword::Order, Keyword::Partition, Keyword::Primary,
    Keyword::Sample, Keyword::Ttl, Keyword::Settings, Keyword::Comment,
    Keyword::As,
];

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
    } else if p.at_keyword(Keyword::Index) {
        parse_create_index(p);
    } else if p.at_keyword(Keyword::User) {
        parse_create_user(p);
    } else if p.at_keyword(Keyword::Role) {
        parse_create_role(p);
    } else if p.at_keyword(Keyword::Quota) {
        parse_create_quota(p);
    } else if p.at_keyword(Keyword::Row) || p.at_keyword(Keyword::Policy) {
        parse_create_row_policy(p);
    } else if (p.at_keyword(Keyword::Settings) && !is_temporary)
        || p.at_keyword(Keyword::Profile)
    {
        parse_create_settings_profile(p);
    } else if is_temporary {
        // TEMPORARY only valid with TABLE
        p.recover_with_error("Expected TABLE after TEMPORARY");
    } else {
        p.advance_with_error("Expected TABLE, DATABASE, VIEW, MATERIALIZED VIEW, FUNCTION, or DICTIONARY");
    }

    p.complete(m, SyntaxKind::CreateStatement);
}

/// Parse CREATE TABLE ...
fn parse_create_table(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Table);

    // IF NOT EXISTS
    common::parse_if_not_exists(p);

    // Table name: [db.]table
    common::parse_table_identifier(p);

    // ON CLUSTER
    common::parse_on_cluster(p);

    // CLONE AS [db.]source_table — copies the source table's schema and data
    // into the new table. ClickHouse optionally accepts ENGINE/ORDER BY/etc.
    // overrides after, so fall through to the standard clause handling.
    if p.at_keyword(Keyword::Clone) {
        let cm = p.start();
        p.advance(); // consume CLONE
        p.expect_keyword(Keyword::As);
        common::parse_table_identifier(p);
        p.complete(cm, SyntaxKind::AsClause);

        common::skip_to_keywords(p, CREATE_TABLE_KEYWORDS);
        if p.at_keyword(Keyword::Engine) {
            parse_engine_clause(p);
        }
        common::skip_to_keywords(p, CREATE_TABLE_KEYWORDS);
        parse_table_clauses(p);
        p.complete(m, SyntaxKind::TableDefinition);
        return;
    }

    // UUID (optional, skip string literal)
    if p.at_keyword(Keyword::As) && !p.at(SyntaxKind::OpeningRoundBracket) {
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
    if p.at(SyntaxKind::OpeningRoundBracket) {
        parse_column_definition_list(p);
    }

    // Skip unexpected tokens before ENGINE
    common::skip_to_keywords(p, CREATE_TABLE_KEYWORDS);

    // ENGINE = ...
    if p.at_keyword(Keyword::Engine) {
        parse_engine_clause(p);
    }

    // Skip unexpected tokens after ENGINE
    common::skip_to_keywords(p, CREATE_TABLE_KEYWORDS);

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
    common::parse_if_not_exists(p);

    // Database name
    if p.at_identifier() {
        p.advance();
    } else if common::at_query_parameter(p) {
        common::parse_query_parameter(p);
    } else {
        p.recover_with_error("Expected database name");
    }

    // ON CLUSTER
    common::parse_on_cluster(p);

    // ENGINE = ...
    if p.at_keyword(Keyword::Engine) {
        parse_engine_clause(p);
    }

    // COMMENT
    if p.at_keyword(Keyword::Comment) {
        let m2 = p.start();
        p.advance(); // COMMENT
        if p.at(SyntaxKind::StringToken) {
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
    common::parse_if_not_exists(p);

    // View name: [db.]view
    common::parse_table_identifier(p);

    // ON CLUSTER
    common::parse_on_cluster(p);

    // Optional column definition list: CREATE VIEW v (col Type, ...) AS SELECT ...
    if p.at(SyntaxKind::OpeningRoundBracket) {
        parse_column_definition_list(p);
    }

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
    common::parse_if_not_exists(p);

    // View name: [db.]view
    common::parse_table_identifier(p);

    // ON CLUSTER
    common::parse_on_cluster(p);

    // TO [db.]table
    if p.at_keyword(Keyword::To) {
        p.advance(); // TO
        common::parse_table_identifier(p);
    }

    // Column definition list (optional)
    if p.at(SyntaxKind::OpeningRoundBracket) {
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

    // IF NOT EXISTS
    common::parse_if_not_exists(p);

    // Function name
    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected function name");
    }

    // AS (args) -> expr  or  AS expr
    p.expect_keyword(Keyword::As);

    // Parse the lambda body: expression, then check for ->
    let lm = p.start();
    parse_expression(p);
    if p.at(SyntaxKind::Arrow) {
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
    common::parse_if_not_exists(p);

    // Dictionary name: [db.]dict
    common::parse_table_identifier(p);

    // ON CLUSTER
    common::parse_on_cluster(p);

    // Column definition list
    if p.at(SyntaxKind::OpeningRoundBracket) {
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

    // Dictionary clauses can appear in any order: SOURCE, LAYOUT, LIFETIME, RANGE
    loop {
        if p.at_keyword(Keyword::Source) {
            parse_dictionary_source(p);
        } else if p.at_keyword(Keyword::Layout) {
            parse_dictionary_layout(p);
        } else if p.at_keyword(Keyword::Lifetime) {
            parse_dictionary_lifetime(p);
        } else if p.at_keyword(Keyword::Range) {
            parse_dictionary_range(p);
        } else {
            break;
        }
    }

    p.complete(m, SyntaxKind::DictionaryDefinition);
}

/// Parse SOURCE(TYPE(key value ...))
/// e.g. SOURCE(CLICKHOUSE(HOST 'localhost' PORT 9000 TABLE 'source_table' DB 'default'))
fn parse_dictionary_source(p: &mut Parser) {
    let m = p.start();
    p.advance(); // SOURCE
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // outer (
        // Source type name (e.g. CLICKHOUSE, MYSQL, FILE, etc.)
        if p.at_identifier() {
            let tm = p.start();
            p.advance(); // type name
            // Inner parens with key-value pairs
            if p.at(SyntaxKind::OpeningRoundBracket) {
                p.advance(); // inner (
                parse_key_value_pairs(p);
                p.expect(SyntaxKind::ClosingRoundBracket); // inner )
            }
            p.complete(tm, SyntaxKind::DictionarySourceType);
        }
        p.expect(SyntaxKind::ClosingRoundBracket); // outer )
    }
    p.complete(m, SyntaxKind::DictionarySource);
}

/// Parse LAYOUT(TYPE(params...))
/// e.g. LAYOUT(HASHED()), LAYOUT(COMPLEX_KEY_HASHED()), LAYOUT(RANGE_HASHED(PREALLOCATE 1))
fn parse_dictionary_layout(p: &mut Parser) {
    let m = p.start();
    p.advance(); // LAYOUT
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // outer (
        // Layout type name (e.g. HASHED, FLAT, COMPLEX_KEY_HASHED, etc.)
        if p.at_identifier() {
            let tm = p.start();
            p.advance(); // type name
            // Inner parens with optional key-value params
            if p.at(SyntaxKind::OpeningRoundBracket) {
                p.advance(); // inner (
                parse_key_value_pairs(p);
                p.expect(SyntaxKind::ClosingRoundBracket); // inner )
            }
            p.complete(tm, SyntaxKind::DictionaryLayoutType);
        }
        p.expect(SyntaxKind::ClosingRoundBracket); // outer )
    }
    p.complete(m, SyntaxKind::DictionaryLayout);
}

/// Parse LIFETIME(number) or LIFETIME(MIN number MAX number)
fn parse_dictionary_lifetime(p: &mut Parser) {
    let m = p.start();
    p.advance(); // LIFETIME
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // (
        if p.at_keyword(Keyword::Min) || p.at_keyword(Keyword::Max) {
            // MIN number MAX number (can appear in either order)
            parse_key_value_pairs(p);
        } else if p.at(SyntaxKind::Number) {
            // Simple form: just a number
            p.advance();
        } else if !p.at(SyntaxKind::ClosingRoundBracket) {
            p.advance_with_error("Expected number or MIN/MAX in LIFETIME");
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
    }
    p.complete(m, SyntaxKind::DictionaryLifetime);
}

/// Parse RANGE(MIN col MAX col)
fn parse_dictionary_range(p: &mut Parser) {
    let m = p.start();
    p.advance(); // RANGE
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // (
        parse_key_value_pairs(p);
        p.expect(SyntaxKind::ClosingRoundBracket);
    }
    p.complete(m, SyntaxKind::DictionaryRange);
}

/// Parse key-value pairs inside dictionary clauses.
/// Each pair is: bareword followed by a literal (string, number, or bareword).
/// e.g. HOST 'localhost' PORT 9000 TABLE 'source_table'
/// or: MIN 300 MAX 600
/// A key can also be followed by a parenthesized block (e.g. STRUCTURE (...))
/// which may contain nested parens for types like Decimal(18, 8).
fn parse_key_value_pairs(p: &mut Parser) {
    while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
        if p.at_identifier() {
            let kv = p.start();
            p.advance(); // key
            // value: string, number, bareword, or parenthesized block
            if p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::Number) {
                p.advance();
            } else if p.at(SyntaxKind::OpeningRoundBracket) {
                // Parenthesized value block, e.g. STRUCTURE (a String, b Decimal(18, 8))
                // Consume everything between matching parens, handling nesting.
                consume_balanced_parens(p);
            } else if p.at_identifier()
                && !p.at(SyntaxKind::ClosingRoundBracket)
            {
                // bareword value (e.g. column name in RANGE)
                p.advance();
            }
            // If no value follows the key, that's OK (flags like PREALLOCATE)
            p.complete(kv, SyntaxKind::DictionaryKeyValue);
        } else {
            // Skip unexpected tokens to avoid infinite loop
            p.advance_with_error("Expected key-value pair in dictionary clause");
        }
    }
}

/// Consume a balanced parenthesized block, including nested parens.
/// Advances from the opening `(` through and including the matching `)`.
fn consume_balanced_parens(p: &mut Parser) {
    assert!(p.at(SyntaxKind::OpeningRoundBracket));
    p.advance(); // consume (
    let mut depth = 1u32;
    while depth > 0 && !p.eof() {
        if p.at(SyntaxKind::OpeningRoundBracket) {
            depth += 1;
        } else if p.at(SyntaxKind::ClosingRoundBracket) {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        p.advance();
    }
    p.expect(SyntaxKind::ClosingRoundBracket);
}

/// Parse AS clause: AS SELECT ... or AS [db.]table
fn parse_as_clause(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::As);

    if at_select_statement(p) {
        parse_select_statement(p);
    } else if p.at(SyntaxKind::OpeningRoundBracket) {
        // Parenthesized subquery: AS (SELECT ...)
        p.advance(); // consume (
        parse_select_statement(p);
        p.expect(SyntaxKind::ClosingRoundBracket);
    } else if p.at_identifier() {
        common::parse_table_identifier(p);
    } else {
        p.recover_with_error("Expected SELECT or table name after AS");
    }

    p.complete(m, SyntaxKind::AsClause);
}

/// Parse the parenthesized column definition list.
pub fn parse_column_definition_list(p: &mut Parser) {
    let m = p.start();

    p.expect(SyntaxKind::OpeningRoundBracket);

    let mut first = true;
    while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        if p.at(SyntaxKind::ClosingRoundBracket) {
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

    p.expect(SyntaxKind::ClosingRoundBracket);

    p.complete(m, SyntaxKind::ColumnDefinitionList);
}

/// Parse a single column definition: name Type [DEFAULT|MATERIALIZED|ALIAS|EPHEMERAL expr] [CODEC(codec)] [TTL expr] [COMMENT 'comment']
fn parse_column_definition(p: &mut Parser) {
    let m = p.start();

    // Column name
    if p.at_identifier() {
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
            && !p.at(SyntaxKind::Comma)
            && !p.at(SyntaxKind::ClosingRoundBracket)
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
        if p.at(SyntaxKind::OpeningRoundBracket) {
            p.advance();
            parse_codec_args(p);
            p.expect(SyntaxKind::ClosingRoundBracket);
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
        if p.at(SyntaxKind::StringToken) {
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
    if !p.at_identifier() {
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
    while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
        if p.at_identifier() {
            p.advance();
            // Optional parameters
            if p.at(SyntaxKind::OpeningRoundBracket) {
                p.advance();
                // Parse codec parameters
                let mut first = true;
                while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
                    if !first {
                        p.expect(SyntaxKind::Comma);
                    }
                    first = false;
                    if !p.at(SyntaxKind::ClosingRoundBracket) {
                        parse_expression(p);
                    }
                }
                p.expect(SyntaxKind::ClosingRoundBracket);
            }
        } else if p.at(SyntaxKind::Comma) {
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
    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected index name");
    }

    // Expression
    parse_expression(p);

    // TYPE type_name
    if p.at_keyword(Keyword::Type) {
        p.advance(); // TYPE
        if p.at_identifier() {
            p.advance();
            // Optional parameters
            if p.at(SyntaxKind::OpeningRoundBracket) {
                p.advance();
                let mut first = true;
                while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
                    if !first {
                        p.expect(SyntaxKind::Comma);
                    }
                    first = false;
                    parse_expression(p);
                }
                p.expect(SyntaxKind::ClosingRoundBracket);
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
    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected projection name");
    }

    // (SELECT ...)
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // (
        if at_select_statement(p) {
            parse_select_statement(p);
        } else {
            p.recover_with_error("Expected SELECT inside PROJECTION");
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
    } else {
        p.recover_with_error("Expected ( after projection name");
    }

    p.complete(m, SyntaxKind::ProjectionDefinition);
}

/// Parse CONSTRAINT definition: CONSTRAINT name CHECK|ASSUME expr
fn parse_constraint_definition(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Constraint);

    // Constraint name
    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected constraint name");
    }

    // CHECK expr / ASSUME expr
    if p.at_keyword(Keyword::Check) || p.at_keyword(Keyword::Assume) {
        p.advance(); // CHECK or ASSUME
        parse_expression(p);
    } else {
        p.recover_with_error("Expected CHECK or ASSUME after constraint name");
    }

    p.complete(m, SyntaxKind::ConstraintDefinition);
}

/// Parse ENGINE = Name(args)
fn parse_engine_clause(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Engine);
    p.eat(SyntaxKind::Equals); // = is optional in ClickHouse

    // Engine name
    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected engine name");
        p.complete(m, SyntaxKind::EngineClause);
        return;
    }

    // Optional arguments
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // (
        let mut first = true;
        while !p.at(SyntaxKind::ClosingRoundBracket) && !p.eof() {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;
            if !p.at(SyntaxKind::ClosingRoundBracket) {
                parse_expression(p);
            }
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
    }

    p.complete(m, SyntaxKind::EngineClause);
}

/// Parse table-level clauses: ORDER BY, PARTITION BY, PRIMARY KEY, SAMPLE BY, TTL, SETTINGS, COMMENT
fn parse_table_clauses(p: &mut Parser) {
    loop {
        common::skip_to_keywords(p, CREATE_TABLE_KEYWORDS);

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
            if p.at(SyntaxKind::StringToken) {
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

        if !first && !p.eat(SyntaxKind::Comma) {
            break;
        }
        first = false;

        common::parse_setting_item(p);
    }

    p.complete(m, SyntaxKind::SettingsClause);
}

// ===========================================================================
// CREATE INDEX
// ===========================================================================

/// Parse: CREATE INDEX [IF NOT EXISTS] name ON [db.]table (column_list) [TYPE type_name] [GRANULARITY number]
fn parse_create_index(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Index);

    // IF NOT EXISTS
    common::parse_if_not_exists(p);

    // Index name
    if p.at_identifier() {
        p.advance();
    } else {
        p.recover_with_error("Expected index name");
    }

    // ON [db.]table
    p.expect_keyword(Keyword::On);
    common::parse_table_identifier(p);

    // (column_list)
    if p.at(SyntaxKind::OpeningRoundBracket) {
        p.advance(); // (
        // Parse comma-separated list of column expressions (may include ASC/DESC)
        let mut first = true;
        while !p.eof() && !p.at(SyntaxKind::ClosingRoundBracket) {
            if !first {
                if p.at(SyntaxKind::Comma) {
                    p.advance();
                } else {
                    break;
                }
            }
            first = false;
            parse_expression(p);
            // Optional ASC/DESC
            let _ = p.eat_keyword(Keyword::Asc);
            let _ = p.eat_keyword(Keyword::Desc);
        }
        p.expect(SyntaxKind::ClosingRoundBracket);
    }

    // Optional TYPE type_name
    if p.at_keyword(Keyword::Type) {
        p.advance(); // TYPE
        if p.at_identifier() {
            p.advance(); // type name
        } else {
            p.recover_with_error("Expected index type name after TYPE");
        }
    }

    // Optional GRANULARITY number
    if p.at_keyword(Keyword::Granularity) {
        p.advance(); // GRANULARITY
        parse_expression(p);
    }

    p.complete(m, SyntaxKind::CreateIndexStatement);
}

// ===========================================================================
// Access control: CREATE USER / ROLE / QUOTA / ROW POLICY / SETTINGS PROFILE
// ===========================================================================

/// Consume remaining tokens until end of statement (generic body for access control).
fn consume_remaining(p: &mut Parser) {
    while !p.eof() && !p.end_of_statement() {
        p.advance();
    }
}

/// Parse: CREATE USER [IF NOT EXISTS] name ... (generic body)
fn parse_create_user(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::User);
    common::parse_if_not_exists(p);
    // User name
    if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
        p.advance();
    }
    // Consume remaining body generically
    consume_remaining(p);
    p.complete(m, SyntaxKind::CreateUserStatement);
}

/// Parse: CREATE ROLE [IF NOT EXISTS] name ...
fn parse_create_role(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Role);
    common::parse_if_not_exists(p);
    if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
        p.advance();
    }
    consume_remaining(p);
    p.complete(m, SyntaxKind::CreateRoleStatement);
}

/// Parse: CREATE QUOTA [IF NOT EXISTS] name ...
fn parse_create_quota(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Quota);
    common::parse_if_not_exists(p);
    if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
        p.advance();
    }
    consume_remaining(p);
    p.complete(m, SyntaxKind::CreateQuotaStatement);
}

/// Parse: CREATE [ROW] POLICY [IF NOT EXISTS] name ON table ...
fn parse_create_row_policy(p: &mut Parser) {
    let m = p.start();
    // Accept both CREATE ROW POLICY and CREATE POLICY
    let _ = p.eat_keyword(Keyword::Row);
    p.expect_keyword(Keyword::Policy);
    common::parse_if_not_exists(p);
    if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
        p.advance();
    }
    consume_remaining(p);
    p.complete(m, SyntaxKind::CreateRowPolicyStatement);
}

/// Parse: CREATE SETTINGS PROFILE / CREATE PROFILE
fn parse_create_settings_profile(p: &mut Parser) {
    let m = p.start();
    // Accept both CREATE SETTINGS PROFILE and CREATE PROFILE
    let _ = p.eat_keyword(Keyword::Settings);
    p.expect_keyword(Keyword::Profile);
    common::parse_if_not_exists(p);
    if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
        p.advance();
    }
    consume_remaining(p);
    p.complete(m, SyntaxKind::CreateSettingsProfileStatement);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    #[test]
    fn test_create_table_basic() {
        let result = parse("CREATE TABLE test (id UInt64, name String) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
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
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("TableIdentifier"));
    }

    #[test]
    fn test_create_table_on_cluster() {
        let result = parse("CREATE TABLE test ON CLUSTER my_cluster (id UInt64) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("OnClusterClause"));
    }

    #[test]
    fn test_create_table_column_defaults() {
        let result = parse("CREATE TABLE test (id UInt64 DEFAULT 0, name String ALIAS 'hello', ts DateTime CODEC(Delta, ZSTD)) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ColumnDefault"));
        assert!(buf.contains("ColumnCodec"));
    }

    #[test]
    fn test_create_table_column_comment() {
        let result = parse("CREATE TABLE test (id UInt64 COMMENT 'primary key') ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ColumnComment"));
    }

    #[test]
    fn test_create_table_index() {
        let result = parse("CREATE TABLE test (id UInt64, INDEX idx id TYPE minmax GRANULARITY 3) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("IndexDefinition"));
    }

    #[test]
    fn test_create_table_constraint() {
        let result = parse("CREATE TABLE test (id UInt64, CONSTRAINT c1 CHECK id > 0) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ConstraintDefinition"));
    }

    #[test]
    fn test_create_table_as_select() {
        let result = parse("CREATE TABLE test AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("AsClause"));
        assert!(buf.contains("SelectStatement"));
    }

    #[test]
    fn test_create_table_as_other_table() {
        let result = parse("CREATE TABLE test AS other_table");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("AsClause"));
        assert!(buf.contains("TableIdentifier"));
    }

    #[test]
    fn test_create_table_with_engine_as_select() {
        let result = parse("CREATE TABLE test ENGINE = MergeTree() ORDER BY id AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("EngineClause"));
        assert!(buf.contains("AsClause"));
    }

    #[test]
    fn test_create_or_replace_table() {
        let result = parse("CREATE OR REPLACE TABLE test (id UInt64) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
    }

    #[test]
    fn test_create_temporary_table() {
        let result = parse("CREATE TEMPORARY TABLE test (id UInt64) ENGINE = Memory()");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
    }

    #[test]
    fn test_create_database() {
        let result = parse("CREATE DATABASE IF NOT EXISTS mydb ENGINE = Atomic() COMMENT 'test db'");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DatabaseDefinition"));
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("EngineClause"));
    }

    #[test]
    fn test_create_view() {
        let result = parse("CREATE VIEW myview AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ViewDefinition"));
        assert!(buf.contains("AsClause"));
    }

    #[test]
    fn test_create_materialized_view() {
        let result = parse("CREATE MATERIALIZED VIEW myview TO dest_table AS SELECT 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("MaterializedViewDefinition"));
    }

    #[test]
    fn test_create_function() {
        let result = parse("CREATE FUNCTION myFunc AS (x) -> x + 1");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("FunctionDefinition"));
        assert!(buf.contains("LambdaExpression"));
    }

    #[test]
    fn test_create_function_if_not_exists() {
        let result = parse("CREATE FUNCTION IF NOT EXISTS myFunc AS (x) -> x + 1");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("FunctionDefinition"));
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("LambdaExpression"));
    }

    #[test]
    fn test_create_function_multi_arg_lambda() {
        let result = parse("CREATE FUNCTION IF NOT EXISTS testfn AS (param_a) -> bitAnd(param_a, 123)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("FunctionDefinition"));
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("LambdaExpression"));
        assert!(buf.contains("FunctionCall"));
    }

    #[test]
    fn test_create_table_all_clauses() {
        let result = parse(
            "CREATE TABLE test (id UInt64) ENGINE = MergeTree() ORDER BY id PARTITION BY id PRIMARY KEY id SAMPLE BY id TTL id SETTINGS index_granularity = 8192 COMMENT 'test'"
        );
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
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
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ProjectionDefinition"));
    }

    #[test]
    fn test_create_table_complex_types() {
        let result = parse("CREATE TABLE test (id UInt64, data Array(String), nested Tuple(a UInt32, b String)) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DataType"));
    }

    #[test]
    fn test_create_table_ttl_column() {
        let result = parse("CREATE TABLE test (id UInt64, ts DateTime TTL ts + 1) ENGINE = MergeTree() ORDER BY id");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ColumnTtl"));
    }

    #[test]
    fn test_error_recovery_missing_engine() {
        // Should parse without panic even without ENGINE
        let result = parse("CREATE TABLE test (id UInt64)");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("CreateStatement"));
        assert!(buf.contains("TableDefinition"));
    }

    #[test]
    fn test_error_recovery_missing_table_name() {
        // Should not panic
        let result = parse("CREATE TABLE");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("CreateStatement"));
    }

    #[test]
    fn test_create_dictionary() {
        let result = parse("CREATE DICTIONARY mydict (id UInt64, name String) PRIMARY KEY id SOURCE(CLICKHOUSE(host 'localhost')) LAYOUT(HASHED()) LIFETIME(300)");
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionaryDefinition"));
        assert!(buf.contains("PrimaryKeyDefinition"));
        assert!(buf.contains("DictionarySource"));
        assert!(buf.contains("DictionarySourceType"));
        assert!(buf.contains("DictionaryLayout"));
        assert!(buf.contains("DictionaryLayoutType"));
        assert!(buf.contains("DictionaryLifetime"));
    }

    #[test]
    fn test_create_dictionary_no_errors() {
        let result = parse("CREATE DICTIONARY mydict (id UInt64, name String) PRIMARY KEY id SOURCE(CLICKHOUSE(HOST 'localhost' PORT 9000 TABLE 'source_table' DB 'default')) LAYOUT(HASHED()) LIFETIME(MIN 300 MAX 600)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
    }

    #[test]
    fn test_create_dictionary_lifetime_simple() {
        let result = parse("CREATE DICTIONARY d (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(FLAT()) LIFETIME(300)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionaryLifetime"));
    }

    #[test]
    fn test_create_dictionary_lifetime_min_max() {
        let result = parse("CREATE DICTIONARY d (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(FLAT()) LIFETIME(MIN 300 MAX 600)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionaryLifetime"));
        assert!(buf.contains("DictionaryKeyValue"));
    }

    #[test]
    fn test_create_dictionary_range() {
        let result = parse("CREATE DICTIONARY d (id UInt64, start Date, end Date) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(RANGE_HASHED()) RANGE(MIN start MAX end) LIFETIME(300)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionaryRange"));
        assert!(buf.contains("DictionaryKeyValue"));
    }

    #[test]
    fn test_create_dictionary_source_key_values() {
        let result = parse("CREATE DICTIONARY d (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE(HOST 'localhost' PORT 9000 TABLE 'src' DB 'default' USER 'admin' PASSWORD 'secret')) LAYOUT(HASHED()) LIFETIME(300)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionarySource"));
        assert!(buf.contains("DictionarySourceType"));
        assert!(buf.contains("DictionaryKeyValue"));
    }

    #[test]
    fn test_create_dictionary_complex_layout() {
        let result = parse("CREATE DICTIONARY d (id UInt64) PRIMARY KEY id SOURCE(CLICKHOUSE()) LAYOUT(COMPLEX_KEY_HASHED()) LIFETIME(0)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionaryLayout"));
        assert!(buf.contains("DictionaryLayoutType"));
    }

    #[test]
    fn test_engine_without_equals() {
        let result = parse("CREATE TABLE t (a Int32) ENGINE MergeTree() ORDER BY a");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("EngineClause"));
        assert!(buf.contains("'MergeTree'"));
    }

    #[test]
    fn test_engine_without_equals_no_parens() {
        let result = parse("CREATE TABLE t (a Int32) ENGINE Memory");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("EngineClause"));
        assert!(buf.contains("'Memory'"));
    }

    #[test]
    fn test_create_view_with_columns() {
        let result = parse("CREATE VIEW v (n Nullable(Int32), f Float64) AS SELECT n, f FROM t");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("ViewDefinition"));
        assert!(buf.contains("ColumnDefinitionList"));
    }

    #[test]
    fn test_create_database_query_parameter() {
        let result = parse("CREATE DATABASE {db:Identifier}");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DatabaseDefinition"));
        assert!(buf.contains("QueryParameterExpression"));
    }

    #[test]
    fn test_create_database_if_not_exists_query_parameter() {
        let result = parse("CREATE DATABASE IF NOT EXISTS {CLICKHOUSE_DATABASE:Identifier}");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("IfNotExistsClause"));
        assert!(buf.contains("QueryParameterExpression"));
    }

    #[test]
    fn test_dictionary_structure_nested_parens() {
        let result = parse("CREATE DICTIONARY dict (a String, b Decimal(18, 8)) PRIMARY KEY a SOURCE(CLICKHOUSE(TABLE '' STRUCTURE (a String b Decimal(18, 8)))) LIFETIME(MIN 0 MAX 0) LAYOUT(FLAT())");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("DictionarySource"));
        assert!(buf.contains("DictionarySourceType"));
        assert!(buf.contains("DictionaryKeyValue"));
    }

    #[test]
    fn test_engine_backtick_args() {
        let result = parse("CREATE TABLE t (a String, b Int64) ENGINE = Join(ANY, LEFT, `a`, `b`)");
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("EngineClause"));
        assert!(buf.contains("'`a`'"));
        assert!(buf.contains("'`b`'"));
    }

    #[test]
    fn test_create_table_clone_as() {
        // CREATE TABLE ... CLONE AS source clones schema + data from source.
        let result = parse(
            "CREATE TABLE new_db.new_tbl CLONE AS src_db.src_tbl",
        );
        assert!(
            result.errors.is_empty(),
            "unexpected errors: {:?}",
            result.errors
        );
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        assert!(buf.contains("AsClause"));
        assert!(buf.contains("'CLONE'"));
        assert!(buf.contains("'src_db'"));
        assert!(buf.contains("'src_tbl'"));
    }

    #[test]
    fn test_create_table_clone_as_with_engine_override() {
        let result = parse(
            "CREATE TABLE t CLONE AS src ENGINE = MergeTree ORDER BY tuple()",
        );
        assert!(
            result.errors.is_empty(),
            "unexpected errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_create_table_clone_as_with_if_not_exists_and_cluster() {
        let result = parse(
            "CREATE TABLE IF NOT EXISTS t ON CLUSTER my_cluster CLONE AS src",
        );
        assert!(
            result.errors.is_empty(),
            "unexpected errors: {:?}",
            result.errors
        );
    }
}
