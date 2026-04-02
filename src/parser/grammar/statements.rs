use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::common;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;

/// Parse optional PARTITION expr, wrapping in PartitionExpression.
fn parse_partition(p: &mut Parser) {
    if p.at_keyword(Keyword::Partition) {
        let m = p.start();
        p.expect_keyword(Keyword::Partition);
        parse_expression(p);
        p.complete(m, SyntaxKind::PartitionExpression);
    }
}

// ---------------------------------------------------------------------------
// USE statement
// ---------------------------------------------------------------------------

pub fn at_use_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Use)
}

pub fn parse_use_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Use);

    if p.at_identifier() {
        p.advance();
    } else if common::at_query_parameter(p) {
        common::parse_query_parameter(p);
    } else {
        p.recover_with_error("Expected database name after USE");
    }

    p.complete(m, SyntaxKind::UseStatement);
}

// ---------------------------------------------------------------------------
// SET statement
// ---------------------------------------------------------------------------

pub fn at_set_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Set)
}

pub fn parse_set_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Set);

    let mut first = true;
    while !p.end_of_statement() {
        if !first {
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        common::parse_setting_item(p);
    }

    p.complete(m, SyntaxKind::SetStatement);
}

// ---------------------------------------------------------------------------
// DROP statement
// ---------------------------------------------------------------------------

pub fn at_drop_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Drop)
}

pub fn parse_drop_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Drop);

    // Check for access control entities: DROP USER/ROLE/QUOTA/ROW POLICY/SETTINGS PROFILE
    if p.at_keyword(Keyword::User)
        || p.at_keyword(Keyword::Role)
        || p.at_keyword(Keyword::Quota)
        || p.at_keyword(Keyword::Row)
        || p.at_keyword(Keyword::Policy)
        || p.at_keyword(Keyword::Profile)
    {
        let inner = p.start();
        // Consume keyword(s) for the entity type
        if p.at_keyword(Keyword::Row) {
            p.advance(); // ROW
        }
        p.advance(); // USER / ROLE / QUOTA / POLICY / PROFILE

        common::parse_if_exists(p);

        // Parse comma-separated name list
        if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
            p.advance();
            while p.at(SyntaxKind::Comma) {
                p.advance(); // comma
                if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
                    p.advance();
                }
            }
        }

        // Consume remaining body (e.g. ON table for ROW POLICY)
        while !p.eof() && !p.end_of_statement() {
            p.advance();
        }
        p.complete(inner, SyntaxKind::DropAccessEntityStatement);
        p.complete(m, SyntaxKind::DropStatement);
        return;
    }

    // Handle DROP SETTINGS PROFILE
    if p.at_keyword(Keyword::Settings) {
        let inner = p.start();
        p.advance(); // SETTINGS
        if p.at_keyword(Keyword::Profile) {
            p.advance(); // PROFILE
        }
        common::parse_if_exists(p);
        if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
            p.advance();
            while p.at(SyntaxKind::Comma) {
                p.advance();
                if p.at_identifier() || p.at(SyntaxKind::StringToken) || p.at(SyntaxKind::QuotedIdentifier) {
                    p.advance();
                }
            }
        }
        while !p.eof() && !p.end_of_statement() {
            p.advance();
        }
        p.complete(inner, SyntaxKind::DropAccessEntityStatement);
        p.complete(m, SyntaxKind::DropStatement);
        return;
    }

    // Optional TEMPORARY
    let _ = p.eat_keyword(Keyword::Temporary);

    // Object kind: TABLE, DATABASE, VIEW, DICTIONARY, FUNCTION
    // TABLE is optional for DROP TABLE
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    } else if p.at_keyword(Keyword::View) {
        p.advance();
    } else if p.at_keyword(Keyword::Dictionary) {
        p.advance();
    } else if p.at_keyword(Keyword::Function) {
        p.advance();
    }
    // If none matched, that's ok -- DROP [IF EXISTS] name is valid shorthand

    common::parse_if_exists(p);

    // Parse the identifier
    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    // Optional PERMANENTLY
    let _ = p.eat_keyword(Keyword::Permanently);

    // Optional SYNC
    let _ = p.eat_keyword(Keyword::Sync);

    p.complete(m, SyntaxKind::DropStatement);
}

// ---------------------------------------------------------------------------
// TRUNCATE statement
// ---------------------------------------------------------------------------

pub fn at_truncate_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Truncate)
}

pub fn parse_truncate_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Truncate);

    // Optional TABLE keyword
    let _ = p.eat_keyword(Keyword::Table);

    common::parse_if_exists(p);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    // Optional SYNC
    let _ = p.eat_keyword(Keyword::Sync);

    p.complete(m, SyntaxKind::TruncateStatement);
}

// ---------------------------------------------------------------------------
// RENAME statement
// ---------------------------------------------------------------------------

pub fn at_rename_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Rename)
}

pub fn parse_rename_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Rename);
    p.expect_keyword(Keyword::Table);

    let mut first = true;
    loop {
        if !first {
            if !p.at(SyntaxKind::Comma) {
                break;
            }
            p.expect(SyntaxKind::Comma);
        }
        first = false;

        parse_rename_item(p);

        if p.end_of_statement() || p.at_keyword(Keyword::On) {
            break;
        }
    }

    common::parse_on_cluster(p);

    p.complete(m, SyntaxKind::RenameStatement);
}

fn parse_rename_item(p: &mut Parser) {
    let m = p.start();

    common::parse_table_identifier(p);

    if p.at_keyword(Keyword::To) {
        p.expect_keyword(Keyword::To);
    } else {
        p.recover_with_error("Expected TO in RENAME");
    }

    common::parse_table_identifier(p);

    p.complete(m, SyntaxKind::RenameItem);
}

// ---------------------------------------------------------------------------
// EXISTS statement
// ---------------------------------------------------------------------------

pub fn at_exists_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Exists)
}

pub fn parse_exists_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Exists);

    // Optional TEMPORARY
    let _ = p.eat_keyword(Keyword::Temporary);

    // Optional object type keyword
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    } else if p.at_keyword(Keyword::View) {
        p.advance();
    } else if p.at_keyword(Keyword::Dictionary) {
        p.advance();
    }

    common::parse_table_identifier(p);

    p.complete(m, SyntaxKind::ExistsStatement);
}

// ---------------------------------------------------------------------------
// CHECK statement
// ---------------------------------------------------------------------------

pub fn at_check_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Check)
}

pub fn parse_check_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Check);
    p.expect_keyword(Keyword::Table);

    common::parse_table_identifier(p);

    parse_partition(p);

    // Optional SETTINGS clause
    common::parse_optional_settings_clause(p);

    p.complete(m, SyntaxKind::CheckStatement);
}

// ---------------------------------------------------------------------------
// OPTIMIZE statement
// ---------------------------------------------------------------------------

pub fn at_optimize_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Optimize)
}

pub fn parse_optimize_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Optimize);
    p.expect_keyword(Keyword::Table);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    parse_partition(p);

    // Optional FINAL [CLEANUP]
    if p.eat_keyword(Keyword::Final) {
        let _ = p.eat_keyword(Keyword::Cleanup);
    }

    // Optional DEDUPLICATE [BY expr, ...]
    if p.at_keyword(Keyword::Deduplicate) {
        p.advance();

        if p.at_keyword(Keyword::By) {
            p.advance();
            // Parse comma-separated expression list
            let m = p.start();
            parse_expression(p);
            while p.at(SyntaxKind::Comma) && !p.end_of_statement() {
                p.advance();
                parse_expression(p);
            }
            p.complete(m, SyntaxKind::IdentifierList);
        }
    }

    // Optional SETTINGS clause
    common::parse_optional_settings_clause(p);

    p.complete(m, SyntaxKind::OptimizeStatement);
}

// ---------------------------------------------------------------------------
// ATTACH statement
// ---------------------------------------------------------------------------

pub fn at_attach_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Attach)
}

pub fn parse_attach_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Attach);

    // Object kind: TABLE, DATABASE
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    }

    common::parse_if_not_exists(p);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    p.complete(m, SyntaxKind::AttachStatement);
}

// ---------------------------------------------------------------------------
// DETACH statement
// ---------------------------------------------------------------------------

pub fn at_detach_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Detach)
}

pub fn parse_detach_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Detach);

    // Object kind: TABLE, DATABASE
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    }

    common::parse_if_exists(p);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    // Optional PERMANENTLY
    let _ = p.eat_keyword(Keyword::Permanently);

    p.complete(m, SyntaxKind::DetachStatement);
}

// ---------------------------------------------------------------------------
// EXCHANGE statement
// ---------------------------------------------------------------------------

pub fn at_exchange_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Exchange)
}

pub fn parse_exchange_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Exchange);
    p.expect_keyword(Keyword::Tables);

    common::parse_table_identifier(p);

    p.expect_keyword(Keyword::And);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    p.complete(m, SyntaxKind::ExchangeStatement);
}

// ---------------------------------------------------------------------------
// UNDROP statement
// ---------------------------------------------------------------------------

pub fn at_undrop_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Undrop)
}

pub fn parse_undrop_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Undrop);
    p.expect_keyword(Keyword::Table);

    common::parse_table_identifier(p);

    common::parse_on_cluster(p);

    p.complete(m, SyntaxKind::UndropStatement);
}

// ---------------------------------------------------------------------------
// BACKUP statement
// ---------------------------------------------------------------------------

pub fn at_backup_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Backup)
}

pub fn parse_backup_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Backup);

    // Object kind: TABLE, DATABASE
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    }

    common::parse_table_identifier(p);

    // TO <expression>
    p.expect_keyword(Keyword::To);
    parse_expression(p);

    // Optional SETTINGS
    if p.at_keyword(Keyword::Settings) {
        let sm = p.start();
        p.expect_keyword(Keyword::Settings);

        let mut first = true;
        while !p.end_of_statement() {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;

            common::parse_setting_item(p);
        }

        p.complete(sm, SyntaxKind::SettingsClause);
    }

    p.complete(m, SyntaxKind::BackupStatement);
}

// ---------------------------------------------------------------------------
// RESTORE statement
// ---------------------------------------------------------------------------

pub fn at_restore_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Restore)
}

pub fn parse_restore_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Restore);

    // Object kind: TABLE, DATABASE
    if p.at_keyword(Keyword::Table) {
        p.advance();
    } else if p.at_keyword(Keyword::Database) {
        p.advance();
    }

    common::parse_table_identifier(p);

    // FROM <expression>
    p.expect_keyword(Keyword::From);
    parse_expression(p);

    // Optional SETTINGS
    if p.at_keyword(Keyword::Settings) {
        let sm = p.start();
        p.expect_keyword(Keyword::Settings);

        let mut first = true;
        while !p.end_of_statement() {
            if !first {
                p.expect(SyntaxKind::Comma);
            }
            first = false;

            common::parse_setting_item(p);
        }

        p.complete(sm, SyntaxKind::SettingsClause);
    }

    p.complete(m, SyntaxKind::RestoreStatement);
}

// GRANT statement
// ---------------------------------------------------------------------------

pub fn at_grant_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Grant)
}

pub fn parse_grant_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Grant);

    // Parse privilege list
    parse_privilege_list(p);

    // ON target
    if p.at_keyword(Keyword::On) {
        parse_grant_target(p);
    } else {
        p.recover_with_error("Expected ON after privilege list");
    }

    // TO user_list
    if p.at_keyword(Keyword::To) {
        p.expect_keyword(Keyword::To);
        parse_identifier_list(p);
    } else {
        p.recover_with_error("Expected TO after grant target");
    }

    // Optional WITH GRANT OPTION
    if p.at_keyword(Keyword::With) {
        p.advance(); // WITH
        let _ = p.eat_keyword(Keyword::Grant);
        let _ = p.eat_keyword(Keyword::Option);
    }

    p.complete(m, SyntaxKind::GrantStatement);
}

// ---------------------------------------------------------------------------
// REVOKE statement
// ---------------------------------------------------------------------------

pub fn at_revoke_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Revoke)
}

pub fn parse_revoke_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Revoke);

    // Parse privilege list
    parse_privilege_list(p);

    // ON target
    if p.at_keyword(Keyword::On) {
        parse_grant_target(p);
    } else {
        p.recover_with_error("Expected ON after privilege list");
    }

    // FROM user_list
    if p.at_keyword(Keyword::From) {
        p.expect_keyword(Keyword::From);
        parse_identifier_list(p);
    } else {
        p.recover_with_error("Expected FROM after revoke target");
    }

    p.complete(m, SyntaxKind::RevokeStatement);
}

// ---------------------------------------------------------------------------
// Shared helpers for GRANT / REVOKE
// ---------------------------------------------------------------------------

/// Parse a comma-separated list of privileges.
/// Each privilege is a keyword/identifier optionally followed by a column list in parens.
/// Example: SELECT, INSERT(col1, col2), ALTER
fn parse_privilege_list(p: &mut Parser) {
    let m = p.start();

    parse_privilege(p);

    while p.at(SyntaxKind::Comma) && !p.end_of_statement() {
        p.advance(); // ','
        parse_privilege(p);
    }

    p.complete(m, SyntaxKind::PrivilegeList);
}

/// Parse a single privilege: a keyword/identifier, optionally followed by
/// additional keyword words (e.g. ALL PRIVILEGES, SHOW TABLES) and
/// an optional column list in parens.
fn parse_privilege(p: &mut Parser) {
    let m = p.start();

    if p.at_identifier() {
        p.advance();

        // Some privileges are multi-word: ALL PRIVILEGES, SHOW TABLES, etc.
        // Consume additional bare words that aren't structural keywords (ON, TO, FROM, comma).
        while p.at(SyntaxKind::BareWord)
            && !p.at_keyword(Keyword::On)
            && !p.at_keyword(Keyword::To)
            && !p.at_keyword(Keyword::From)
            && !p.end_of_statement()
        {
            // Don't consume if next is comma (separates privileges)
            if p.at(SyntaxKind::Comma) {
                break;
            }
            p.advance();
        }

        // Optional column list: SELECT(col1, col2)
        if p.at(SyntaxKind::OpeningRoundBracket) {
            p.advance(); // '('
            parse_identifier_list(p);
            p.expect(SyntaxKind::ClosingRoundBracket);
        }
    } else {
        p.recover_with_error("Expected privilege name");
    }

    p.complete(m, SyntaxKind::Privilege);
}

/// Parse ON target: ON db.table, ON db.*, ON *.*
fn parse_grant_target(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::On);

    // Parse the target: could be *.*, db.*, db.table, or just *
    if p.at(SyntaxKind::Star) {
        p.advance(); // '*'
        if p.at(SyntaxKind::Dot) {
            p.advance(); // '.'
            if p.at(SyntaxKind::Star) {
                p.advance(); // '*'
            } else if p.at_identifier() {
                p.advance();
            } else {
                p.recover_with_error("Expected table name or * after dot");
            }
        }
    } else if p.at_identifier() {
        p.advance(); // db or table name
        if p.at(SyntaxKind::Dot) {
            p.advance(); // '.'
            if p.at(SyntaxKind::Star) {
                p.advance(); // '*'
            } else if p.at_identifier() {
                p.advance();
            } else {
                p.recover_with_error("Expected table name or * after dot");
            }
        }
    } else {
        p.recover_with_error("Expected target after ON");
    }

    p.complete(m, SyntaxKind::GrantTarget);
}

/// Parse comma-separated identifier list (for user names, column names in privileges).
fn parse_identifier_list(p: &mut Parser) {
    if p.at_identifier() {
        p.advance();
    }

    while p.at(SyntaxKind::Comma) && !p.end_of_statement() {
        p.advance(); // ','
        if p.at_identifier() {
            p.advance();
        } else {
            p.recover_with_error("Expected identifier");
        }
    }
}
// ---------------------------------------------------------------------------
// SYSTEM statement
// ---------------------------------------------------------------------------

pub fn at_system_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::System)
}

pub fn parse_system_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::System);

    // Start SystemCommand node — consume subcommand keyword tokens.
    //
    // Strategy: first consume the action keyword (DROP/RELOAD/FLUSH/STOP/
    // START/SYNC), then greedily consume all following barewords that are
    // part of the subcommand name. We stop when we see something that
    // looks like a table identifier (a bareword followed by a dot or end
    // of statement) or a FOR keyword, or end of statement.
    let cmd = p.start();

    // Track whether this is a FLUSH LOGS or SYNC REPLICA command
    let mut is_flush_logs = false;
    let mut is_sync_replica = false;
    let mut prev_was_flush = false;
    let mut prev_was_sync = false;

    // First, consume the action keyword
    let has_action = p.at_keyword(Keyword::Reload)
        || p.at_keyword(Keyword::Drop)
        || p.at_keyword(Keyword::Flush)
        || p.at_keyword(Keyword::Stop)
        || p.at_keyword(Keyword::Start)
        || p.at_keyword(Keyword::Sync);

    if has_action {
        prev_was_flush = p.at_keyword(Keyword::Flush);
        prev_was_sync = p.at_keyword(Keyword::Sync);
        p.advance();
    }

    // Now greedily consume subsequent barewords as part of the command name.
    // We stop when we see:
    //   - end of statement
    //   - FOR keyword (used in "SYSTEM DROP FORMAT SCHEMA CACHE FOR Protobuf")
    //   - a bareword followed by a dot (table identifier like db.table)
    //   - a bareword that looks like it's the table argument (i.e., the next
    //     token after it is end-of-statement, comma, or semicolon — but only
    //     after we've consumed at least one subcommand word after the action)
    let mut subcommand_word_count = 0;
    loop {
        if p.end_of_statement() {
            break;
        }

        // FOR keyword terminates the command name
        if p.at_keyword(Keyword::For) {
            break;
        }

        // If not a bareword, stop
        if !p.at(SyntaxKind::BareWord) {
            break;
        }

        // If this bareword is followed by a dot, it's a table identifier
        if p.nth(1) == SyntaxKind::Dot {
            break;
        }

        // Track FLUSH LOGS and SYNC REPLICA patterns
        if p.at_keyword(Keyword::Logs) && prev_was_flush {
            is_flush_logs = true;
        }
        if p.at_keyword(Keyword::Replica) && prev_was_sync {
            is_sync_replica = true;
        }
        prev_was_flush = p.at_keyword(Keyword::Flush);
        prev_was_sync = p.at_keyword(Keyword::Sync);

        // After consuming at least one subcommand word, if the next bareword
        // is followed by end-of-statement or comma, AND we already have a
        // recognized terminal keyword (like CACHE, LOGS, etc.), we should
        // check if this word IS the terminal keyword. If it's a known
        // terminal keyword, consume it. Otherwise it's likely a table name.
        let is_known_terminal = p.at_keyword(Keyword::Cache)
            || p.at_keyword(Keyword::Logs)
            || p.at_keyword(Keyword::Dictionaries)
            || p.at_keyword(Keyword::Dictionary)
            || p.at_keyword(Keyword::Config)
            || p.at_keyword(Keyword::Functions)
            || p.at_keyword(Keyword::Replica)
            || p.at_keyword(Keyword::Replicas);

        // Recognize words that form SYSTEM subcommand names. We use a
        // text-level check for words that are not yet first-class Keywords
        // (e.g. CONDITION, SCHEMA, REPLICATION, QUEUES).
        let text_upper: String = p.nth_text(0).to_ascii_uppercase();
        let is_text_system_word = matches!(
            text_upper.as_str(),
            "CONDITION" | "SCHEMA" | "REPLICATION" | "QUEUES"
        );

        let is_known_modifier = is_text_system_word
            || p.at_keyword(Keyword::Dns)
            || p.at_keyword(Keyword::Mark)
            || p.at_keyword(Keyword::Uncompressed)
            || p.at_keyword(Keyword::Compiled)
            || p.at_keyword(Keyword::Distributed)
            || p.at_keyword(Keyword::Merges)
            || p.at_keyword(Keyword::Sends)
            || p.at_keyword(Keyword::Replicated)
            || p.at_keyword(Keyword::Fetches)
            || p.at_keyword(Keyword::Moves)
            || p.at_keyword(Keyword::Query)
            || p.at_keyword(Keyword::Format)
            || p.at_keyword(Keyword::Drop);

        if !is_known_terminal && !is_known_modifier && subcommand_word_count > 0 {
            // This bareword is not a recognized system subcommand keyword
            // and we already have subcommand words — treat it as a table name
            break;
        }

        p.advance();
        subcommand_word_count += 1;
    }

    p.complete(cmd, SyntaxKind::SystemCommand);

    // Handle FOR keyword (e.g., SYSTEM DROP FORMAT SCHEMA CACHE FOR Protobuf)
    if p.at_keyword(Keyword::For) {
        p.advance(); // FOR
        if p.at_identifier() {
            p.advance(); // protocol name (Protobuf, HDFS, etc.)
        }
    }

    if is_flush_logs {
        // FLUSH LOGS can have comma-separated log target list: query_log, trace_log
        while !p.end_of_statement() {
            if p.at_identifier() {
                p.advance();
            } else if p.at(SyntaxKind::Comma) {
                p.advance();
            } else {
                break;
            }
        }
    } else if is_sync_replica {
        // SYNC REPLICA takes a table identifier then optional trailing keywords (PULL, STRICT, etc.)
        if !p.end_of_statement() {
            common::parse_table_identifier(p);
        }
        // Consume trailing modifier keywords (PULL, STRICT, etc.)
        while !p.end_of_statement() && p.at_identifier() {
            p.advance();
        }
    } else {
        // Optionally parse table identifier for commands that take one
        if !p.end_of_statement() {
            common::parse_table_identifier(p);
        }
    }

    p.complete(m, SyntaxKind::SystemStatement);
}

// ---------------------------------------------------------------------------
// KILL statement
// ---------------------------------------------------------------------------

pub fn at_kill_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Kill)
}

pub fn parse_kill_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Kill);

    // QUERY or MUTATION — wrap in KillTarget
    let target = p.start();
    if p.at_keyword(Keyword::Query) {
        p.advance();
    } else if p.at_keyword(Keyword::Mutation) {
        p.advance();
    } else {
        p.recover_with_error("Expected QUERY or MUTATION after KILL");
    }
    p.complete(target, SyntaxKind::KillTarget);

    // WHERE expression
    if p.at_keyword(Keyword::Where) {
        let wm = p.start();
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
        p.complete(wm, SyntaxKind::WhereClause);
    }

    // Optional SYNC/ASYNC/TEST keyword
    let _ = p.eat_keyword(Keyword::Sync)
        || p.eat_keyword(Keyword::Async)
        || p.eat_keyword(Keyword::Test);

    p.complete(m, SyntaxKind::KillStatement);
}

// ---------------------------------------------------------------------------
// BEGIN TRANSACTION / COMMIT / ROLLBACK
// ---------------------------------------------------------------------------

pub fn at_begin_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Begin)
}

pub fn parse_begin_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Begin);
    // Optional TRANSACTION keyword
    let _ = p.eat_keyword(Keyword::Transaction);
    p.complete(m, SyntaxKind::BeginStatement);
}

pub fn at_commit_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Commit)
}

pub fn parse_commit_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Commit);
    p.complete(m, SyntaxKind::CommitStatement);
}

pub fn at_rollback_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Rollback)
}

pub fn parse_rollback_statement(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Rollback);
    p.complete(m, SyntaxKind::RollbackStatement);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use expect_test::{expect, Expect};

    fn check(input: &str, expected_tree: Expect) {
        let result = parse(input);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        expected_tree.assert_eq(&buf);
    }

    #[test]
    fn test_use_statement() {
        check(
            "USE mydb",
            expect![[r#"
                File
                  UseStatement
                    'USE'
                    'mydb'
            "#]],
        );
    }

    #[test]
    fn test_use_statement_missing_db() {
        check(
            "USE",
            expect![[r#"
                File
                  UseStatement
                    'USE'
                    Error
            "#]],
        );
    }

    #[test]
    fn test_set_single() {
        check(
            "SET max_threads = 4",
            expect![[r#"
                File
                  SetStatement
                    'SET'
                    SettingItem
                      'max_threads'
                      '='
                      NumberLiteral
                        '4'
            "#]],
        );
    }

    #[test]
    fn test_set_multiple() {
        check(
            "SET max_threads = 4, max_memory_usage = 1000000",
            expect![[r#"
                File
                  SetStatement
                    'SET'
                    SettingItem
                      'max_threads'
                      '='
                      NumberLiteral
                        '4'
                    ','
                    SettingItem
                      'max_memory_usage'
                      '='
                      NumberLiteral
                        '1000000'
            "#]],
        );
    }

    #[test]
    fn test_drop_table() {
        check(
            "DROP TABLE mydb.mytable",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_drop_table_if_exists() {
        check(
            "DROP TABLE IF EXISTS mydb.mytable",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_drop_database_on_cluster() {
        check(
            "DROP DATABASE IF EXISTS mydb ON CLUSTER mycluster",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'DATABASE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'mydb'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_drop_temporary_table() {
        check(
            "DROP TEMPORARY TABLE tmp",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TEMPORARY'
                    'TABLE'
                    TableIdentifier
                      'tmp'
            "#]],
        );
    }

    #[test]
    fn test_drop_view() {
        check(
            "DROP VIEW IF EXISTS myview",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'VIEW'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'myview'
            "#]],
        );
    }

    #[test]
    fn test_drop_permanently() {
        check(
            "DROP TABLE mytable PERMANENTLY",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'PERMANENTLY'
            "#]],
        );
    }

    #[test]
    fn test_drop_sync() {
        check(
            "DROP TABLE mytable SYNC",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'SYNC'
            "#]],
        );
    }

    #[test]
    fn test_truncate_table() {
        check(
            "TRUNCATE TABLE mydb.mytable",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_truncate_without_table_keyword() {
        check(
            "TRUNCATE mydb.mytable",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_truncate_if_exists_on_cluster() {
        check(
            "TRUNCATE TABLE IF EXISTS mytable ON CLUSTER mycluster",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    'TABLE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'mytable'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_truncate_sync() {
        check(
            "TRUNCATE TABLE default.records SYNC",
            expect![[r#"
                File
                  TruncateStatement
                    'TRUNCATE'
                    'TABLE'
                    TableIdentifier
                      'default'
                      '.'
                      'records'
                    'SYNC'
            "#]],
        );
    }

    #[test]
    fn test_rename_table() {
        check(
            "RENAME TABLE old TO new",
            expect![[r#"
                File
                  RenameStatement
                    'RENAME'
                    'TABLE'
                    RenameItem
                      TableIdentifier
                        'old'
                      'TO'
                      TableIdentifier
                        'new'
            "#]],
        );
    }

    #[test]
    fn test_rename_multiple() {
        check(
            "RENAME TABLE db.old1 TO db.new1, db.old2 TO db.new2",
            expect![[r#"
                File
                  RenameStatement
                    'RENAME'
                    'TABLE'
                    RenameItem
                      TableIdentifier
                        'db'
                        '.'
                        'old1'
                      'TO'
                      TableIdentifier
                        'db'
                        '.'
                        'new1'
                    ','
                    RenameItem
                      TableIdentifier
                        'db'
                        '.'
                        'old2'
                      'TO'
                      TableIdentifier
                        'db'
                        '.'
                        'new2'
            "#]],
        );
    }

    #[test]
    fn test_rename_on_cluster() {
        check(
            "RENAME TABLE old TO new ON CLUSTER mycluster",
            expect![[r#"
                File
                  RenameStatement
                    'RENAME'
                    'TABLE'
                    RenameItem
                      TableIdentifier
                        'old'
                      'TO'
                      TableIdentifier
                        'new'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_exists_table() {
        check(
            "EXISTS TABLE mydb.mytable",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_exists_database() {
        check(
            "EXISTS DATABASE mydb",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    'DATABASE'
                    TableIdentifier
                      'mydb'
            "#]],
        );
    }

    #[test]
    fn test_exists_no_keyword() {
        check(
            "EXISTS mytable",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    TableIdentifier
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_exists_temporary() {
        check(
            "EXISTS TEMPORARY TABLE tmp",
            expect![[r#"
                File
                  ExistsStatement
                    'EXISTS'
                    'TEMPORARY'
                    'TABLE'
                    TableIdentifier
                      'tmp'
            "#]],
        );
    }

    #[test]
    fn test_check_table() {
        check(
            "CHECK TABLE mydb.mytable",
            expect![[r#"
                File
                  CheckStatement
                    'CHECK'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_check_table_partition() {
        check(
            "CHECK TABLE mytable PARTITION 202301",
            expect![[r#"
                File
                  CheckStatement
                    'CHECK'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    PartitionExpression
                      'PARTITION'
                      NumberLiteral
                        '202301'
            "#]],
        );
    }

    #[test]
    fn test_optimize_table() {
        check(
            "OPTIMIZE TABLE mydb.mytable",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mydb'
                      '.'
                      'mytable'
            "#]],
        );
    }

    #[test]
    fn test_optimize_final() {
        check(
            "OPTIMIZE TABLE mytable FINAL",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'FINAL'
            "#]],
        );
    }

    #[test]
    fn test_optimize_deduplicate() {
        check(
            "OPTIMIZE TABLE mytable DEDUPLICATE",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    'DEDUPLICATE'
            "#]],
        );
    }

    #[test]
    fn test_optimize_full() {
        check(
            "OPTIMIZE TABLE mytable ON CLUSTER mycluster PARTITION 202301 FINAL DEDUPLICATE",
            expect![[r#"
                File
                  OptimizeStatement
                    'OPTIMIZE'
                    'TABLE'
                    TableIdentifier
                      'mytable'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
                    PartitionExpression
                      'PARTITION'
                      NumberLiteral
                        '202301'
                    'FINAL'
                    'DEDUPLICATE'
            "#]],
        );
    }

    #[test]
    fn test_set_string_value() {
        check(
            "SET log_comment = 'my test'",
            expect![[r#"
                File
                  SetStatement
                    'SET'
                    SettingItem
                      'log_comment'
                      '='
                      StringLiteral
                        ''my test''
            "#]],
        );
    }

    #[test]
    fn test_drop_function() {
        check(
            "DROP FUNCTION IF EXISTS my_func ON CLUSTER mycluster",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'FUNCTION'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'my_func'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      'mycluster'
            "#]],
        );
    }

    #[test]
    fn test_drop_dictionary() {
        check(
            "DROP DICTIONARY mydict",
            expect![[r#"
                File
                  DropStatement
                    'DROP'
                    'DICTIONARY'
                    TableIdentifier
                      'mydict'
            "#]],
        );
    }

    #[test]
    fn test_attach_table() {
        check(
            "ATTACH TABLE IF NOT EXISTS db.t ON CLUSTER 'c'",
            expect![[r#"
                File
                  AttachStatement
                    'ATTACH'
                    'TABLE'
                    IfNotExistsClause
                      'IF'
                      'NOT'
                      'EXISTS'
                    TableIdentifier
                      'db'
                      '.'
                      't'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      ''c''
            "#]],
        );
    }

    #[test]
    fn test_detach_table_permanently() {
        check(
            "DETACH TABLE IF EXISTS db.t PERMANENTLY",
            expect![[r#"
                File
                  DetachStatement
                    'DETACH'
                    'TABLE'
                    IfExistsClause
                      'IF'
                      'EXISTS'
                    TableIdentifier
                      'db'
                      '.'
                      't'
                    'PERMANENTLY'
            "#]],
        );
    }

    #[test]
    fn test_exchange_tables() {
        check(
            "EXCHANGE TABLES t1 AND t2 ON CLUSTER 'c'",
            expect![[r#"
                File
                  ExchangeStatement
                    'EXCHANGE'
                    'TABLES'
                    TableIdentifier
                      't1'
                    'AND'
                    TableIdentifier
                      't2'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      ''c''
            "#]],
        );
    }

    #[test]
    fn test_undrop_table() {
        check(
            "UNDROP TABLE db.t ON CLUSTER 'c'",
            expect![[r#"
                File
                  UndropStatement
                    'UNDROP'
                    'TABLE'
                    TableIdentifier
                      'db'
                      '.'
                      't'
                    OnClusterClause
                      'ON'
                      'CLUSTER'
                      ''c''
            "#]],
        );
    }

    #[test]
    fn test_backup_table() {
        check(
            "BACKUP TABLE db.t TO Disk('backups', '1.zip')",
            expect![[r#"
                File
                  BackupStatement
                    'BACKUP'
                    'TABLE'
                    TableIdentifier
                      'db'
                      '.'
                      't'
                    'TO'
                    FunctionCall
                      Identifier
                        'Disk'
                      ExpressionList
                        '('
                        Expression
                          StringLiteral
                            ''backups''
                        ','
                        Expression
                          StringLiteral
                            ''1.zip''
                        ')'
            "#]],
        );
    }

    #[test]
    fn test_restore_table_with_settings() {
        check(
            "RESTORE TABLE db.t FROM Disk('backups', '1.zip') SETTINGS allow_non_empty_tables = true",
            expect![[r#"
                File
                  RestoreStatement
                    'RESTORE'
                    'TABLE'
                    TableIdentifier
                      'db'
                      '.'
                      't'
                    'FROM'
                    FunctionCall
                      Identifier
                        'Disk'
                      ExpressionList
                        '('
                        Expression
                          StringLiteral
                            ''backups''
                        ','
                        Expression
                          StringLiteral
                            ''1.zip''
                        ')'
                    SettingsClause
                      'SETTINGS'
                      SettingItem
                        'allow_non_empty_tables'
                        '='
                        BooleanLiteral
                          'true'
            "#]],
        );
    }

    #[test]
    fn test_revoke_select() {
        check(
            "REVOKE SELECT ON db.t FROM user1",
            expect![[r#"
                File
                  RevokeStatement
                    'REVOKE'
                    PrivilegeList
                      Privilege
                        'SELECT'
                    GrantTarget
                      'ON'
                      'db'
                      '.'
                      't'
                    'FROM'
                    'user1'
            "#]],
        );
    }

    #[test]
    fn test_system_reload_dictionary_with_table() {
        check(
            "SYSTEM RELOAD DICTIONARY db.mydict",
            expect![[r#"
                File
                  SystemStatement
                    'SYSTEM'
                    SystemCommand
                      'RELOAD'
                      'DICTIONARY'
                    TableIdentifier
                      'db'
                      '.'
                      'mydict'
            "#]],
        );
    }

    #[test]
    fn test_system_drop_dns_cache() {
        check(
            "SYSTEM DROP DNS CACHE",
            expect![[r#"
                File
                  SystemStatement
                    'SYSTEM'
                    SystemCommand
                      'DROP'
                      'DNS'
                      'CACHE'
            "#]],
        );
    }

    #[test]
    fn test_system_flush_distributed() {
        check(
            "SYSTEM FLUSH DISTRIBUTED db.t",
            expect![[r#"
                File
                  SystemStatement
                    'SYSTEM'
                    SystemCommand
                      'FLUSH'
                      'DISTRIBUTED'
                    TableIdentifier
                      'db'
                      '.'
                      't'
            "#]],
        );
    }

    #[test]
    fn test_revoke_all_privileges() {
        check(
            "REVOKE ALL PRIVILEGES ON *.* FROM user1",
            expect![[r#"
                File
                  RevokeStatement
                    'REVOKE'
                    PrivilegeList
                      Privilege
                        'ALL'
                        'PRIVILEGES'
                    GrantTarget
                      'ON'
                      '*'
                      '.'
                      '*'
                    'FROM'
                    'user1'
            "#]],
        );
    }

    #[test]
    fn test_system_sync_replica() {
        check(
            "SYSTEM SYNC REPLICA db.t",
            expect![[r#"
                File
                  SystemStatement
                    'SYSTEM'
                    SystemCommand
                      'SYNC'
                      'REPLICA'
                    TableIdentifier
                      'db'
                      '.'
                      't'
            "#]],
        );
    }

    // -----------------------------------------------------------------------
    // KILL statement tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_kill_query() {
        check(
            "KILL QUERY WHERE query_id = '123'",
            expect![[r#"
                File
                  KillStatement
                    'KILL'
                    KillTarget
                      'QUERY'
                    WhereClause
                      'WHERE'
                      BinaryExpression
                        ColumnReference
                          'query_id'
                        '='
                        StringLiteral
                          ''123''
            "#]],
        );
    }

    #[test]
    fn test_kill_query_sync() {
        check(
            "KILL QUERY WHERE query_id = '123' SYNC",
            expect![[r#"
                File
                  KillStatement
                    'KILL'
                    KillTarget
                      'QUERY'
                    WhereClause
                      'WHERE'
                      BinaryExpression
                        ColumnReference
                          'query_id'
                        '='
                        StringLiteral
                          ''123''
                    'SYNC'
            "#]],
        );
    }

    #[test]
    fn test_kill_mutation() {
        check(
            "KILL MUTATION WHERE mutation_id = '456'",
            expect![[r#"
                File
                  KillStatement
                    'KILL'
                    KillTarget
                      'MUTATION'
                    WhereClause
                      'WHERE'
                      BinaryExpression
                        ColumnReference
                          'mutation_id'
                        '='
                        StringLiteral
                          ''456''
            "#]],
        );
    }
}
