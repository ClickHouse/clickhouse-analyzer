pub mod alter;
pub mod common;
pub mod create_table;
pub mod delete;
pub mod expressions;
pub mod insert;
pub mod select;
pub mod show;
pub mod statements;
pub mod types;

use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::grammar::alter::{at_alter_statement, parse_alter_statement};
use crate::parser::grammar::create_table::{at_create_statement, parse_create_statement};
use crate::parser::grammar::delete::{at_delete_statement, parse_delete_statement};
use crate::parser::grammar::insert::{at_insert_statement, parse_insert_statement};
use crate::parser::grammar::select::{at_select_statement, parse_select_statement};
use crate::parser::grammar::show::{
    at_describe_statement, at_explain_statement, at_show_statement, parse_describe_statement,
    parse_explain_statement, parse_show_statement,
};
use crate::parser::grammar::statements::*;
use crate::parser::parser::Parser;

/// Top-level grammar entry point. Parses a full source file containing
/// one or more semicolon-separated SQL statements.
pub fn parse_source(p: &mut Parser) {
    let m = p.start();

    while !p.eof() {
        if at_insert_statement(p) {
            parse_insert_statement(p);
        } else if at_explain_statement(p) {
            parse_explain_statement(p);
        } else if at_describe_statement(p) {
            parse_describe_statement(p);
        } else if at_show_statement(p) {
            parse_show_statement(p);
        } else if at_alter_statement(p) {
            parse_alter_statement(p);
        } else if at_delete_statement(p) {
            parse_delete_statement(p);
        } else if at_create_statement(p) {
            parse_create_statement(p);
        } else if at_use_statement(p) {
            parse_use_statement(p);
        } else if at_set_statement(p) {
            parse_set_statement(p);
        } else if at_drop_statement(p) {
            parse_drop_statement(p);
        } else if at_truncate_statement(p) {
            parse_truncate_statement(p);
        } else if at_rename_statement(p) {
            parse_rename_statement(p);
        } else if at_exists_statement(p) {
            parse_exists_statement(p);
        } else if at_check_statement(p) {
            parse_check_statement(p);
        } else if at_optimize_statement(p) {
            parse_optimize_statement(p);
        } else if at_attach_statement(p) {
            parse_attach_statement(p);
        } else if at_detach_statement(p) {
            parse_detach_statement(p);
        } else if at_exchange_statement(p) {
            parse_exchange_statement(p);
        } else if at_undrop_statement(p) {
            parse_undrop_statement(p);
        } else if at_backup_statement(p) {
            parse_backup_statement(p);
        } else if at_restore_statement(p) {
            parse_restore_statement(p);
        } else if at_grant_statement(p) {
            parse_grant_statement(p);
        } else if at_revoke_statement(p) {
            parse_revoke_statement(p);
        } else if at_system_statement(p) {
            parse_system_statement(p);
        } else if at_kill_statement(p) {
            parse_kill_statement(p);
        } else if at_begin_statement(p) {
            parse_begin_statement(p);
        } else if at_commit_statement(p) {
            parse_commit_statement(p);
        } else if at_rollback_statement(p) {
            parse_rollback_statement(p);
        } else if at_select_statement(p) {
            parse_select_statement(p);
        } else if p.at(SyntaxKind::Semicolon) {
            p.advance();
        } else if !p.eof() {
            p.advance_with_error("Unexpected token");
        }
    }

    p.skip_trivia();

    p.complete(m, SyntaxKind::File);
}
