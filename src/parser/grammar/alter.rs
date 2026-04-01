use crate::lexer::token::TokenKind;
use crate::parser::grammar::expressions::parse_expression;
use crate::parser::grammar::types::parse_column_type;
use crate::parser::keyword::Keyword;
use crate::parser::parser::Parser;
use crate::parser::syntax_kind::SyntaxKind;

pub fn at_alter_statement(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::Alter)
}

pub fn parse_alter_statement(p: &mut Parser) {
    let m = p.start();

    p.expect_keyword(Keyword::Alter);
    p.expect_keyword(Keyword::Table);

    // Parse [db.]table
    parse_table_identifier(p);

    // Parse optional ON CLUSTER
    if p.at_keyword(Keyword::On) {
        parse_on_cluster(p);
    }

    // Parse command list
    parse_alter_command_list(p);

    p.complete(m, SyntaxKind::AlterStatement);
}

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
        p.recover_with_error("Expected table name");
    }

    p.complete(m, SyntaxKind::TableIdentifier);
}

fn parse_on_cluster(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::On);
    p.expect_keyword(Keyword::Cluster);

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier, TokenKind::StringLiteral]) {
        p.advance();
    } else {
        p.recover_with_error("Expected cluster name");
    }

    p.complete(m, SyntaxKind::OnClusterClause);
}

fn parse_if_exists(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::If);
    p.expect_keyword(Keyword::Exists);
    p.complete(m, SyntaxKind::IfExistsClause);
}

fn parse_if_not_exists(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::If);
    p.expect_keyword(Keyword::Not);
    p.expect_keyword(Keyword::Exists);
    p.complete(m, SyntaxKind::IfNotExistsClause);
}

/// Returns true if currently at IF EXISTS or IF NOT EXISTS
fn at_if_clause(p: &mut Parser) -> bool {
    p.at_keyword(Keyword::If)
}

fn parse_alter_command_list(p: &mut Parser) {
    let m = p.start();

    let mut first = true;
    while !p.end_of_statement() {
        if !first {
            if p.at(TokenKind::Comma) {
                p.advance();
            } else {
                break;
            }
        }
        first = false;

        parse_alter_command(p);
    }

    p.complete(m, SyntaxKind::AlterCommandList);
}

fn parse_alter_command(p: &mut Parser) {
    if p.at_keyword(Keyword::Add) {
        parse_add_command(p);
    } else if p.at_keyword(Keyword::Drop) {
        parse_drop_command(p);
    } else if p.at_keyword(Keyword::Modify) {
        parse_modify_command(p);
    } else if p.at_keyword(Keyword::Rename) {
        parse_rename_command(p);
    } else if p.at_keyword(Keyword::Clear) {
        parse_clear_command(p);
    } else if p.at_keyword(Keyword::Comment) {
        parse_comment_column(p);
    } else if p.at_keyword(Keyword::Detach) {
        parse_detach_partition(p);
    } else if p.at_keyword(Keyword::Attach) {
        parse_attach_partition(p);
    } else if p.at_keyword(Keyword::Freeze) {
        parse_freeze_partition(p);
    } else if p.at_keyword(Keyword::Delete) {
        parse_delete_where(p);
    } else if p.at_keyword(Keyword::Update) {
        parse_update_where(p);
    } else if p.at_keyword(Keyword::Materialize) {
        parse_materialize_index(p);
    } else if p.at_keyword(Keyword::Reset) {
        parse_reset_setting(p);
    } else {
        // Unknown command - wrap in error and skip to next comma or end
        skip_unknown_command(p);
    }
}

fn skip_unknown_command(p: &mut Parser) {
    let m = p.start();
    if !p.eof() {
        p.advance();
    }
    // Skip tokens until we reach a comma or end of statement
    while !p.at(TokenKind::Comma) && !p.end_of_statement() {
        p.advance();
    }
    p.complete(m, SyntaxKind::Error);
}

// ---- ADD commands ----

fn parse_add_command(p: &mut Parser) {
    // Peek at what follows ADD
    // We need to look ahead past ADD to see COLUMN, INDEX, PROJECTION, CONSTRAINT
    let m = p.start();
    p.expect_keyword(Keyword::Add);

    if p.at_keyword(Keyword::Column) {
        p.advance(); // consume COLUMN

        if at_if_clause(p) {
            parse_if_not_exists(p);
        }

        parse_column_definition(p);

        // AFTER col | FIRST
        if p.at_keyword(Keyword::After) {
            p.advance();
            if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
                p.advance();
            } else {
                p.recover_with_error("Expected column name after AFTER");
            }
        } else if p.at_keyword(Keyword::First) {
            p.advance();
        }

        p.complete(m, SyntaxKind::AlterAddColumn);
    } else if p.at_keyword(Keyword::Index) {
        p.advance(); // consume INDEX

        if at_if_clause(p) {
            parse_if_not_exists(p);
        }

        // index_name
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected index name");
        }

        // expr
        parse_expression(p);

        // TYPE type
        p.expect_keyword(Keyword::Type);
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected index type");
        }

        // Optional parameters in parens
        if p.at(TokenKind::OpeningRoundBracket) {
            p.advance();
            while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
                p.advance();
            }
            p.expect(TokenKind::ClosingRoundBracket);
        }

        // GRANULARITY val
        p.expect_keyword(Keyword::Granularity);
        if p.at(TokenKind::Number) {
            p.advance();
        } else {
            p.recover_with_error("Expected granularity value");
        }

        // AFTER index | FIRST
        if p.at_keyword(Keyword::After) {
            p.advance();
            if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
                p.advance();
            } else {
                p.recover_with_error("Expected index name after AFTER");
            }
        } else if p.at_keyword(Keyword::First) {
            p.advance();
        }

        p.complete(m, SyntaxKind::AlterAddIndex);
    } else if p.at_keyword(Keyword::Projection) {
        p.advance(); // consume PROJECTION

        if at_if_clause(p) {
            parse_if_not_exists(p);
        }

        // projection_name
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected projection name");
        }

        // (SELECT ...)
        if p.at(TokenKind::OpeningRoundBracket) {
            p.advance();
            parse_select_statement(p);
            p.expect(TokenKind::ClosingRoundBracket);
        } else {
            p.recover_with_error("Expected opening parenthesis for projection definition");
        }

        p.complete(m, SyntaxKind::AlterAddProjection);
    } else if p.at_keyword(Keyword::Constraint) {
        p.advance(); // consume CONSTRAINT

        if at_if_clause(p) {
            parse_if_not_exists(p);
        }

        // constraint_name
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected constraint name");
        }

        // CHECK expr
        p.expect_keyword(Keyword::Check);
        parse_expression(p);

        p.complete(m, SyntaxKind::AlterAddConstraint);
    } else {
        // Unknown ADD target
        p.advance_with_error("Expected COLUMN, INDEX, PROJECTION, or CONSTRAINT after ADD");
        p.complete(m, SyntaxKind::Error);
    }
}

// ---- DROP commands ----

fn parse_drop_command(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Drop);

    if p.at_keyword(Keyword::Column) {
        p.advance(); // consume COLUMN

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        // column name
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected column name");
        }

        p.complete(m, SyntaxKind::AlterDropColumn);
    } else if p.at_keyword(Keyword::Index) {
        p.advance(); // consume INDEX

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected index name");
        }

        p.complete(m, SyntaxKind::AlterDropIndex);
    } else if p.at_keyword(Keyword::Projection) {
        p.advance(); // consume PROJECTION

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected projection name");
        }

        p.complete(m, SyntaxKind::AlterDropProjection);
    } else if p.at_keyword(Keyword::Constraint) {
        p.advance(); // consume CONSTRAINT

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected constraint name");
        }

        p.complete(m, SyntaxKind::AlterDropConstraint);
    } else if p.at_keyword(Keyword::Partition) || p.at_keyword(Keyword::Part) {
        p.advance(); // consume PARTITION or PART
        parse_partition_expression(p);
        p.complete(m, SyntaxKind::AlterDropPartition);
    } else {
        p.advance_with_error("Expected COLUMN, INDEX, PROJECTION, CONSTRAINT, PARTITION, or PART after DROP");
        p.complete(m, SyntaxKind::Error);
    }
}

// ---- MODIFY commands ----

fn parse_modify_command(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Modify);

    if p.at_keyword(Keyword::Column) {
        p.advance(); // consume COLUMN

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        parse_column_definition(p);

        p.complete(m, SyntaxKind::AlterModifyColumn);
    } else if p.at_keyword(Keyword::Order) {
        p.advance(); // consume ORDER
        p.expect_keyword(Keyword::By);
        parse_expression(p);
        p.complete(m, SyntaxKind::AlterModifyOrderBy);
    } else if p.at_keyword(Keyword::Ttl) {
        p.advance(); // consume TTL
        parse_expression(p);
        p.complete(m, SyntaxKind::AlterModifyTtl);
    } else if p.at_keyword(Keyword::Setting) || p.at_keyword(Keyword::Settings) {
        p.advance(); // consume SETTING/SETTINGS
        parse_setting_list(p);
        p.complete(m, SyntaxKind::AlterModifySetting);
    } else {
        p.advance_with_error("Expected COLUMN, ORDER BY, TTL, or SETTING after MODIFY");
        p.complete(m, SyntaxKind::Error);
    }
}

// ---- RENAME COLUMN ----

fn parse_rename_command(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Rename);
    p.expect_keyword(Keyword::Column);

    if at_if_clause(p) {
        parse_if_exists(p);
    }

    // old name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected column name");
    }

    p.expect_keyword(Keyword::To);

    // new name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected new column name");
    }

    p.complete(m, SyntaxKind::AlterRenameColumn);
}

// ---- CLEAR commands ----

fn parse_clear_command(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Clear);

    if p.at_keyword(Keyword::Column) {
        p.advance(); // consume COLUMN

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected column name");
        }

        // Optional IN PARTITION
        if p.at_keyword(Keyword::In) {
            p.advance();
            p.expect_keyword(Keyword::Partition);
            parse_partition_expression(p);
        }

        p.complete(m, SyntaxKind::AlterClearColumn);
    } else if p.at_keyword(Keyword::Index) {
        p.advance(); // consume INDEX

        if at_if_clause(p) {
            parse_if_exists(p);
        }

        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected index name");
        }

        // Optional IN PARTITION
        if p.at_keyword(Keyword::In) {
            p.advance();
            p.expect_keyword(Keyword::Partition);
            parse_partition_expression(p);
        }

        p.complete(m, SyntaxKind::AlterClearIndex);
    } else {
        p.advance_with_error("Expected COLUMN or INDEX after CLEAR");
        p.complete(m, SyntaxKind::Error);
    }
}

// ---- COMMENT COLUMN ----

fn parse_comment_column(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Comment);
    p.expect_keyword(Keyword::Column);

    if at_if_clause(p) {
        parse_if_exists(p);
    }

    // column name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected column name");
    }

    // comment string
    if p.at(TokenKind::StringLiteral) {
        p.advance();
    } else {
        p.recover_with_error("Expected comment string");
    }

    p.complete(m, SyntaxKind::AlterCommentColumn);
}

// ---- MATERIALIZE INDEX ----

fn parse_materialize_index(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Materialize);
    p.expect_keyword(Keyword::Index);

    if at_if_clause(p) {
        parse_if_exists(p);
    }

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected index name");
    }

    // Optional IN PARTITION
    if p.at_keyword(Keyword::In) {
        p.advance();
        p.expect_keyword(Keyword::Partition);
        parse_partition_expression(p);
    }

    p.complete(m, SyntaxKind::AlterMaterializeIndex);
}

// ---- Partition commands ----

fn parse_partition_expression(p: &mut Parser) {
    let m = p.start();
    parse_expression(p);
    p.complete(m, SyntaxKind::PartitionExpression);
}

fn parse_detach_partition(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Detach);

    if p.at_keyword(Keyword::Partition) || p.at_keyword(Keyword::Part) {
        p.advance();
    } else {
        p.recover_with_error("Expected PARTITION or PART after DETACH");
    }

    parse_partition_expression(p);

    p.complete(m, SyntaxKind::AlterDetachPartition);
}

fn parse_attach_partition(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Attach);

    if p.at_keyword(Keyword::Partition) || p.at_keyword(Keyword::Part) {
        p.advance();
    } else {
        p.recover_with_error("Expected PARTITION or PART after ATTACH");
    }

    parse_partition_expression(p);

    // Optional FROM [db.]table
    if p.at_keyword(Keyword::From) {
        p.advance();
        parse_table_identifier(p);
    }

    p.complete(m, SyntaxKind::AlterAttachPartition);
}

fn parse_freeze_partition(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Freeze);

    // PARTITION is optional for FREEZE
    if p.at_keyword(Keyword::Partition) {
        p.advance();
        parse_partition_expression(p);
    }

    p.complete(m, SyntaxKind::AlterFreezePartition);
}

// ---- DELETE WHERE ----

fn parse_delete_where(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Delete);

    if p.at_keyword(Keyword::Where) {
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
    } else {
        p.recover_with_error("Expected WHERE after DELETE");
    }

    p.complete(m, SyntaxKind::AlterDeleteWhere);
}

// ---- UPDATE ... WHERE ----

fn parse_update_where(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Update);

    // Parse assignment list: col = expr [, col = expr ...]
    parse_assignment_list(p);

    if p.at_keyword(Keyword::Where) {
        p.expect_keyword(Keyword::Where);
        parse_expression(p);
    } else {
        p.recover_with_error("Expected WHERE after UPDATE assignments");
    }

    p.complete(m, SyntaxKind::AlterUpdateWhere);
}

fn parse_assignment_list(p: &mut Parser) {
    let m = p.start();

    let mut first = true;
    loop {
        if !first {
            if p.at(TokenKind::Comma) {
                p.advance();
            } else {
                break;
            }
        }
        first = false;

        // Stop if we hit WHERE or end of statement
        if p.at_keyword(Keyword::Where) || p.end_of_statement() {
            break;
        }

        parse_assignment(p);
    }

    p.complete(m, SyntaxKind::AssignmentList);
}

fn parse_assignment(p: &mut Parser) {
    let m = p.start();

    // column name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected column name in assignment");
    }

    p.expect(TokenKind::Equals);
    parse_expression(p);

    p.complete(m, SyntaxKind::Assignment);
}

// ---- RESET SETTING ----

fn parse_reset_setting(p: &mut Parser) {
    let m = p.start();
    p.expect_keyword(Keyword::Reset);
    p.expect_keyword(Keyword::Setting);

    // key [, key ...]
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected setting name");
    }

    while p.at(TokenKind::Comma) && !p.end_of_statement() {
        p.advance();
        if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
            p.advance();
        } else {
            p.recover_with_error("Expected setting name");
        }
    }

    p.complete(m, SyntaxKind::AlterResetSetting);
}

// ---- Setting list for MODIFY SETTING ----

fn parse_setting_list(p: &mut Parser) {
    parse_setting_item(p);

    while p.at(TokenKind::Comma) && !p.end_of_statement() {
        p.advance();
        parse_setting_item(p);
    }
}

fn parse_setting_item(p: &mut Parser) {
    let m = p.start();

    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected setting name");
    }

    p.expect(TokenKind::Equals);
    parse_expression(p);

    p.complete(m, SyntaxKind::SettingItem);
}

// ---- Column definition for ADD COLUMN / MODIFY COLUMN ----

fn parse_column_definition(p: &mut Parser) {
    let m = p.start();

    // column name
    if p.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier]) {
        p.advance();
    } else {
        p.recover_with_error("Expected column name");
    }

    // column type
    parse_column_type(p);

    // Optional DEFAULT|MATERIALIZED|ALIAS expr
    if p.at_keyword(Keyword::Default)
        || p.at_keyword(Keyword::Materialized)
        || p.at_keyword(Keyword::Alias)
    {
        p.advance();
        parse_expression(p);
    }

    // Optional CODEC(...)
    if p.at_keyword(Keyword::Codec) {
        p.advance();
        if p.at(TokenKind::OpeningRoundBracket) {
            p.advance();
            while !p.at(TokenKind::ClosingRoundBracket) && !p.eof() {
                p.advance();
            }
            p.expect(TokenKind::ClosingRoundBracket);
        }
    }

    // Optional TTL expr
    if p.at_keyword(Keyword::Ttl) {
        p.advance();
        parse_expression(p);
    }

    // Optional COMMENT 'string'
    if p.at_keyword(Keyword::Comment) {
        p.advance();
        if p.at(TokenKind::StringLiteral) {
            p.advance();
        } else {
            p.recover_with_error("Expected comment string");
        }
    }

    p.complete(m, SyntaxKind::ColumnDefinition);
}

fn parse_select_statement(p: &mut Parser) {
    crate::parser::grammar::select::parse_select_statement(p);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    fn parse_to_string(input: &str) -> String {
        let result = parse(input);
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        buf
    }

    #[test]
    fn test_alter_add_column() {
        let result = parse_to_string("ALTER TABLE t ADD COLUMN c Int32");
        assert!(result.contains("AlterStatement"), "Expected AlterStatement in:\n{result}");
        assert!(result.contains("AlterAddColumn"), "Expected AlterAddColumn in:\n{result}");
        assert!(result.contains("ColumnDefinition"), "Expected ColumnDefinition in:\n{result}");
        assert!(result.contains("DataType"), "Expected DataType in:\n{result}");
    }

    #[test]
    fn test_alter_add_column_if_not_exists() {
        let result = parse_to_string("ALTER TABLE t ADD COLUMN IF NOT EXISTS c String DEFAULT 'hello'");
        assert!(result.contains("AlterAddColumn"), "Expected AlterAddColumn in:\n{result}");
        assert!(result.contains("IfNotExistsClause"), "Expected IfNotExistsClause in:\n{result}");
        assert!(result.contains("ColumnDefinition"), "Expected ColumnDefinition in:\n{result}");
    }

    #[test]
    fn test_alter_drop_column() {
        let result = parse_to_string("ALTER TABLE t DROP COLUMN c");
        assert!(result.contains("AlterDropColumn"), "Expected AlterDropColumn in:\n{result}");
    }

    #[test]
    fn test_alter_drop_column_if_exists() {
        let result = parse_to_string("ALTER TABLE t DROP COLUMN IF EXISTS c");
        assert!(result.contains("AlterDropColumn"), "Expected AlterDropColumn in:\n{result}");
        assert!(result.contains("IfExistsClause"), "Expected IfExistsClause in:\n{result}");
    }

    #[test]
    fn test_alter_rename_column() {
        let result = parse_to_string("ALTER TABLE t RENAME COLUMN old_col TO new_col");
        assert!(result.contains("AlterRenameColumn"), "Expected AlterRenameColumn in:\n{result}");
        assert!(result.contains("'old_col'"), "Expected old_col in:\n{result}");
        assert!(result.contains("'new_col'"), "Expected new_col in:\n{result}");
    }

    #[test]
    fn test_alter_modify_column() {
        let result = parse_to_string("ALTER TABLE t MODIFY COLUMN c String");
        assert!(result.contains("AlterModifyColumn"), "Expected AlterModifyColumn in:\n{result}");
        assert!(result.contains("ColumnDefinition"), "Expected ColumnDefinition in:\n{result}");
    }

    #[test]
    fn test_alter_delete_where() {
        let result = parse_to_string("ALTER TABLE t DELETE WHERE x > 5");
        assert!(result.contains("AlterDeleteWhere"), "Expected AlterDeleteWhere in:\n{result}");
        assert!(result.contains("BinaryExpression"), "Expected BinaryExpression in:\n{result}");
    }

    #[test]
    fn test_alter_update_where() {
        let result = parse_to_string("ALTER TABLE t UPDATE x = 1 WHERE y > 5");
        assert!(result.contains("AlterUpdateWhere"), "Expected AlterUpdateWhere in:\n{result}");
        assert!(result.contains("AssignmentList"), "Expected AssignmentList in:\n{result}");
        assert!(result.contains("Assignment"), "Expected Assignment in:\n{result}");
    }

    #[test]
    fn test_alter_multiple_commands() {
        let result = parse_to_string("ALTER TABLE t ADD COLUMN a Int32, DROP COLUMN b");
        assert!(result.contains("AlterAddColumn"), "Expected AlterAddColumn in:\n{result}");
        assert!(result.contains("AlterDropColumn"), "Expected AlterDropColumn in:\n{result}");
        assert!(result.contains("AlterCommandList"), "Expected AlterCommandList in:\n{result}");
    }

    #[test]
    fn test_alter_on_cluster() {
        let result = parse_to_string("ALTER TABLE db.t ON CLUSTER my_cluster DROP COLUMN c");
        assert!(result.contains("OnClusterClause"), "Expected OnClusterClause in:\n{result}");
        assert!(result.contains("AlterDropColumn"), "Expected AlterDropColumn in:\n{result}");
        assert!(result.contains("'my_cluster'"), "Expected cluster name in:\n{result}");
    }

    #[test]
    fn test_alter_drop_partition() {
        let result = parse_to_string("ALTER TABLE t DROP PARTITION 202301");
        assert!(result.contains("AlterDropPartition"), "Expected AlterDropPartition in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_freeze_partition() {
        let result = parse_to_string("ALTER TABLE t FREEZE PARTITION 202301");
        assert!(result.contains("AlterFreezePartition"), "Expected AlterFreezePartition in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_freeze_no_partition() {
        let result = parse_to_string("ALTER TABLE t FREEZE");
        assert!(result.contains("AlterFreezePartition"), "Expected AlterFreezePartition in:\n{result}");
        // No PartitionExpression expected
        assert!(!result.contains("PartitionExpression"), "Should NOT have PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_modify_order_by() {
        let result = parse_to_string("ALTER TABLE t MODIFY ORDER BY (a, b)");
        assert!(result.contains("AlterModifyOrderBy"), "Expected AlterModifyOrderBy in:\n{result}");
        assert!(result.contains("TupleExpression"), "Expected TupleExpression in:\n{result}");
    }

    #[test]
    fn test_alter_modify_ttl() {
        let result = parse_to_string("ALTER TABLE t MODIFY TTL d + 1");
        assert!(result.contains("AlterModifyTtl"), "Expected AlterModifyTtl in:\n{result}");
        assert!(result.contains("BinaryExpression"), "Expected BinaryExpression in:\n{result}");
    }

    #[test]
    fn test_alter_modify_setting() {
        let result = parse_to_string("ALTER TABLE t MODIFY SETTING key1 = 1");
        assert!(result.contains("AlterModifySetting"), "Expected AlterModifySetting in:\n{result}");
        assert!(result.contains("SettingItem"), "Expected SettingItem in:\n{result}");
    }

    #[test]
    fn test_alter_reset_setting() {
        let result = parse_to_string("ALTER TABLE t RESET SETTING key1, key2");
        assert!(result.contains("AlterResetSetting"), "Expected AlterResetSetting in:\n{result}");
        assert!(result.contains("'key1'"), "Expected key1 in:\n{result}");
        assert!(result.contains("'key2'"), "Expected key2 in:\n{result}");
    }

    #[test]
    fn test_alter_comment_column() {
        let result = parse_to_string("ALTER TABLE t COMMENT COLUMN c 'this is a comment'");
        assert!(result.contains("AlterCommentColumn"), "Expected AlterCommentColumn in:\n{result}");
    }

    #[test]
    fn test_alter_clear_column_in_partition() {
        let result = parse_to_string("ALTER TABLE t CLEAR COLUMN c IN PARTITION 202301");
        assert!(result.contains("AlterClearColumn"), "Expected AlterClearColumn in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_detach_partition() {
        let result = parse_to_string("ALTER TABLE t DETACH PARTITION 202301");
        assert!(result.contains("AlterDetachPartition"), "Expected AlterDetachPartition in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_attach_partition_from() {
        let result = parse_to_string("ALTER TABLE t ATTACH PARTITION 202301 FROM other_table");
        assert!(result.contains("AlterAttachPartition"), "Expected AlterAttachPartition in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
        assert!(result.contains("'other_table'"), "Expected other_table in:\n{result}");
    }

    #[test]
    fn test_alter_add_constraint() {
        let result = parse_to_string("ALTER TABLE t ADD CONSTRAINT c1 CHECK x > 0");
        assert!(result.contains("AlterAddConstraint"), "Expected AlterAddConstraint in:\n{result}");
        assert!(result.contains("BinaryExpression"), "Expected BinaryExpression in:\n{result}");
    }

    #[test]
    fn test_alter_drop_constraint() {
        let result = parse_to_string("ALTER TABLE t DROP CONSTRAINT c1");
        assert!(result.contains("AlterDropConstraint"), "Expected AlterDropConstraint in:\n{result}");
    }

    #[test]
    fn test_alter_add_index() {
        let result = parse_to_string("ALTER TABLE t ADD INDEX idx1 col1 TYPE minmax GRANULARITY 3");
        assert!(result.contains("AlterAddIndex"), "Expected AlterAddIndex in:\n{result}");
        assert!(result.contains("'idx1'"), "Expected idx1 in:\n{result}");
    }

    #[test]
    fn test_alter_drop_index() {
        let result = parse_to_string("ALTER TABLE t DROP INDEX idx1");
        assert!(result.contains("AlterDropIndex"), "Expected AlterDropIndex in:\n{result}");
    }

    #[test]
    fn test_alter_clear_index_in_partition() {
        let result = parse_to_string("ALTER TABLE t CLEAR INDEX idx1 IN PARTITION 202301");
        assert!(result.contains("AlterClearIndex"), "Expected AlterClearIndex in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_materialize_index() {
        let result = parse_to_string("ALTER TABLE t MATERIALIZE INDEX idx1 IN PARTITION 202301");
        assert!(result.contains("AlterMaterializeIndex"), "Expected AlterMaterializeIndex in:\n{result}");
        assert!(result.contains("PartitionExpression"), "Expected PartitionExpression in:\n{result}");
    }

    #[test]
    fn test_alter_drop_projection() {
        let result = parse_to_string("ALTER TABLE t DROP PROJECTION proj1");
        assert!(result.contains("AlterDropProjection"), "Expected AlterDropProjection in:\n{result}");
    }

    #[test]
    fn test_alter_no_error_on_valid_input() {
        let result = parse_to_string("ALTER TABLE t ADD COLUMN c Int32");
        assert!(!result.contains("Error"), "Should not contain Error in:\n{result}");
    }

    #[test]
    fn test_alter_unknown_command_recovers() {
        // Unknown command should produce an Error but not panic
        let result = parse_to_string("ALTER TABLE t UNKNOWN_CMD");
        assert!(result.contains("AlterStatement"), "Expected AlterStatement in:\n{result}");
        assert!(result.contains("Error"), "Expected error recovery in:\n{result}");
    }

    #[test]
    fn test_alter_database_dot_table() {
        let result = parse_to_string("ALTER TABLE mydb.mytable DROP COLUMN c");
        assert!(result.contains("TableIdentifier"), "Expected TableIdentifier in:\n{result}");
        assert!(result.contains("'mydb'"), "Expected database name in:\n{result}");
        assert!(result.contains("'mytable'"), "Expected table name in:\n{result}");
    }

    #[test]
    fn test_alter_add_column_with_after() {
        let result = parse_to_string("ALTER TABLE t ADD COLUMN c Int32 AFTER b");
        assert!(result.contains("AlterAddColumn"), "Expected AlterAddColumn in:\n{result}");
        assert!(result.contains("'b'"), "Expected AFTER column name in:\n{result}");
    }

    #[test]
    fn test_alter_add_column_first() {
        let result = parse_to_string("ALTER TABLE t ADD COLUMN c Int32 FIRST");
        assert!(result.contains("AlterAddColumn"), "Expected AlterAddColumn in:\n{result}");
        assert!(result.contains("'FIRST'"), "Expected FIRST keyword in:\n{result}");
    }

    #[test]
    fn test_alter_update_multiple_assignments() {
        let result = parse_to_string("ALTER TABLE t UPDATE x = 1, y = 2 WHERE z > 0");
        assert!(result.contains("AlterUpdateWhere"), "Expected AlterUpdateWhere in:\n{result}");
        assert!(result.contains("AssignmentList"), "Expected AssignmentList in:\n{result}");
    }
}
