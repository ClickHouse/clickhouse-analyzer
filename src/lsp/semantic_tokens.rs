use tower_lsp::lsp_types::*;

use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};

use super::line_index::LineIndex;

/// The token types we report, in the order they appear in the legend.
/// The index into this array is used as the token type ID in the encoded data.
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,    // 0
    SemanticTokenType::FUNCTION,   // 1
    SemanticTokenType::STRING,     // 2
    SemanticTokenType::NUMBER,     // 3
    SemanticTokenType::COMMENT,    // 4
    SemanticTokenType::OPERATOR,   // 5
    SemanticTokenType::VARIABLE,   // 6  — column references
    SemanticTokenType::TYPE,       // 7  — data types
    SemanticTokenType::PARAMETER,  // 8  — query parameters {param:Type}
    SemanticTokenType::PROPERTY,   // 9  — table/database names
];

pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[];

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: TOKEN_MODIFIERS.to_vec(),
    }
}

const TT_KEYWORD: u32 = 0;
const TT_FUNCTION: u32 = 1;
const TT_STRING: u32 = 2;
const TT_NUMBER: u32 = 3;
const TT_COMMENT: u32 = 4;
const TT_OPERATOR: u32 = 5;
const TT_VARIABLE: u32 = 6;
const TT_TYPE: u32 = 7;
const TT_PARAMETER: u32 = 8;
const TT_PROPERTY: u32 = 9;

/// Walk the CST and produce LSP semantic tokens.
pub fn compute(tree: &SyntaxTree, source: &str, line_index: &LineIndex) -> Vec<SemanticToken> {
    let mut raw_tokens: Vec<(u32, u32, u32)> = Vec::new(); // (start_byte, end_byte, type)
    collect_tokens(tree, source, &mut raw_tokens);

    // Sort by position (should already be in order, but be safe).
    raw_tokens.sort_by_key(|t| t.0);

    // Encode as LSP delta format.
    let mut result = Vec::with_capacity(raw_tokens.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for (start, end, token_type) in raw_tokens {
        let pos = line_index.position(start);
        let length = end - start;

        let delta_line = pos.line - prev_line;
        let delta_start = if delta_line == 0 {
            pos.character - prev_start
        } else {
            pos.character
        };

        result.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        prev_line = pos.line;
        prev_start = pos.character;
    }

    result
}

fn collect_tokens(tree: &SyntaxTree, source: &str, out: &mut Vec<(u32, u32, u32)>) {
    for child in &tree.children {
        match child {
            SyntaxChild::Token(token) => {
                if let Some(tt) = classify_token(token.kind, token.start, token.end, source, tree.kind) {
                    out.push((token.start, token.end, tt));
                }
            }
            SyntaxChild::Tree(subtree) => {
                collect_tokens(subtree, source, out);
            }
        }
    }
}

/// Classify a token based on its own kind and the parent tree node kind.
fn classify_token(
    kind: SyntaxKind,
    start: u32,
    end: u32,
    source: &str,
    parent: SyntaxKind,
) -> Option<u32> {
    match kind {
        // Unambiguous token kinds
        SyntaxKind::Comment => Some(TT_COMMENT),
        SyntaxKind::Number => Some(TT_NUMBER),
        SyntaxKind::StringToken | SyntaxKind::HereDoc => Some(TT_STRING),

        // Operators
        SyntaxKind::Plus
        | SyntaxKind::Minus
        | SyntaxKind::Star
        | SyntaxKind::Slash
        | SyntaxKind::Percent
        | SyntaxKind::Equals
        | SyntaxKind::NotEquals
        | SyntaxKind::Less
        | SyntaxKind::Greater
        | SyntaxKind::LessOrEquals
        | SyntaxKind::GreaterOrEquals
        | SyntaxKind::Spaceship
        | SyntaxKind::Concatenation
        | SyntaxKind::DoubleColon
        | SyntaxKind::Arrow => Some(TT_OPERATOR),

        // Quoted identifiers — context-dependent
        SyntaxKind::QuotedIdentifier => classify_identifier_by_parent(parent),

        // BareWord — the big one. Context determines everything.
        SyntaxKind::BareWord => {
            let text = &source[start as usize..end as usize];
            classify_bareword(text, parent)
        }

        // Skip whitespace, brackets, punctuation, etc.
        _ => None,
    }
}

/// Classify a BareWord by checking if it's a keyword (case-insensitive) in the
/// context of its parent tree node.
fn classify_bareword(text: &str, parent: SyntaxKind) -> Option<u32> {
    // If parent is a data type node, this is a type name
    if matches!(parent, SyntaxKind::DataType | SyntaxKind::DataTypeParameters | SyntaxKind::NestedDataType) {
        return Some(TT_TYPE);
    }

    // If parent is a function call or aggregate, the first bareword is the function name,
    // but we can't distinguish first vs. other here without position info.
    // Use parent kind: FunctionCall and AggregateFunction children that are BareWords
    // before the opening paren are function names. Since we don't have sibling info,
    // classify all BareWords inside function-name-bearing parents by checking if it's a keyword.
    // If it's not a keyword and parent is FunctionCall/AggregateFunction/TableFunction, it's a function.
    if matches!(
        parent,
        SyntaxKind::FunctionCall | SyntaxKind::AggregateFunction | SyntaxKind::TableFunction
    ) {
        if !is_keyword(text) {
            return Some(TT_FUNCTION);
        }
    }

    // Table/database identifiers
    if matches!(
        parent,
        SyntaxKind::TableIdentifier
            | SyntaxKind::TableExpression
            | SyntaxKind::DatabaseDefinition
            | SyntaxKind::TableDefinition
            | SyntaxKind::ViewDefinition
            | SyntaxKind::MaterializedViewDefinition
            | SyntaxKind::DictionaryDefinition
            | SyntaxKind::TableAlias
    ) {
        if !is_keyword(text) {
            return Some(TT_PROPERTY);
        }
    }

    // Column aliases
    if matches!(parent, SyntaxKind::ColumnAlias) {
        if !is_keyword(text) {
            return Some(TT_VARIABLE);
        }
    }

    // Column references
    if matches!(
        parent,
        SyntaxKind::ColumnReference | SyntaxKind::QualifiedName | SyntaxKind::Identifier
    ) {
        if !is_keyword(text) {
            return Some(TT_VARIABLE);
        }
    }

    // Query parameters
    if matches!(parent, SyntaxKind::QueryParameterExpression) {
        return Some(TT_PARAMETER);
    }

    // If it's a known keyword, mark it as such regardless of context
    if is_keyword(text) {
        return Some(TT_KEYWORD);
    }

    // Fallback: identifiers in expression context that we didn't match above
    // are likely column references
    if matches!(parent, SyntaxKind::Expression) {
        return Some(TT_VARIABLE);
    }

    None
}

fn classify_identifier_by_parent(parent: SyntaxKind) -> Option<u32> {
    match parent {
        SyntaxKind::FunctionCall | SyntaxKind::AggregateFunction | SyntaxKind::TableFunction => {
            Some(TT_FUNCTION)
        }
        SyntaxKind::DataType | SyntaxKind::DataTypeParameters | SyntaxKind::NestedDataType => {
            Some(TT_TYPE)
        }
        SyntaxKind::TableIdentifier
        | SyntaxKind::TableExpression
        | SyntaxKind::TableAlias
        | SyntaxKind::DatabaseDefinition
        | SyntaxKind::TableDefinition => Some(TT_PROPERTY),
        SyntaxKind::ColumnReference
        | SyntaxKind::QualifiedName
        | SyntaxKind::ColumnAlias
        | SyntaxKind::Identifier => Some(TT_VARIABLE),
        _ => Some(TT_VARIABLE), // quoted identifiers are usually column/table refs
    }
}

/// Check if a bareword is a SQL keyword (case-insensitive).
fn is_keyword(text: &str) -> bool {
    // Fast path: check length bounds (shortest keyword is 2 chars: AS, BY, IF, IN, IS, ON, OR, TO)
    if text.len() < 2 {
        return false;
    }
    KEYWORDS.contains(&text.to_ascii_uppercase().as_str())
}

/// All known ClickHouse SQL keywords. This set is used purely for syntax highlighting
/// to distinguish keywords from identifiers. It doesn't need to be exhaustive — missing
/// a keyword just means it gets colored as an identifier.
static KEYWORDS: phf::Set<&'static str> = phf::phf_set! {
    "ADD", "AFTER", "ALIAS", "ALL", "ALTER", "AND", "ANTI", "ANY", "ARRAY", "AS",
    "ASC", "ASOF", "ASSUME", "ASYNC", "ATTACH", "BACKUP", "BEGIN", "BETWEEN", "BY",
    "CACHE", "CASE", "CAST", "CHECK", "CLEANUP", "CLEAR", "CLUSTER", "CODEC",
    "COLUMN", "COLUMNS", "COMMENT", "COMMIT", "COMPILED", "COMPLEX", "CONFIG",
    "CONSTRAINT", "CREATE", "CROSS", "CUBE", "CURRENT", "DATABASE", "DATABASES",
    "DEDUPLICATE", "DEFAULT", "DELETE", "DESC", "DESCRIBE", "DETACH", "DICTIONARIES",
    "DICTIONARY", "DIRECT", "DISKS", "DISTINCT", "DISTRIBUTED", "DIV", "DNS", "DROP",
    "ELSE", "EMPTY", "END", "ENGINE", "ENGINES", "EPHEMERAL", "ESTIMATE", "EXCEPT",
    "EXCHANGE", "EXISTS", "EXPLAIN", "FALSE", "FETCH", "FETCHES", "FILL", "FILTER",
    "FINAL", "FIRST", "FLAT", "FLUSH", "FOLLOWING", "FOR", "FORMAT", "FREEZE", "FROM",
    "FULL", "FUNCTION", "FUNCTIONS", "GLOBAL", "GRANT", "GRANTS", "GRANULARITY",
    "GROUP", "GROUPING", "GROUPS", "HASHED", "HAVING", "HIERARCHICAL", "HOST",
    "ID", "IDENTIFIED", "IF", "IGNORE", "ILIKE", "IN", "INDEX", "INJECTIVE", "INNER",
    "INSERT", "INTERPOLATE", "INTERSECT", "INTERVAL", "INTO", "IS", "ISNULL", "JOIN",
    "KEY", "KEYED", "KILL", "LAST", "LAYOUT", "LEFT", "LIFETIME", "LIKE", "LIMIT",
    "LIVE", "LOCAL", "LOGS", "MARK", "MATERIALIZE", "MATERIALIZED", "MAX",
    "MERGES", "MIN", "MOD", "MODELS", "MODIFY", "MOVE", "MOVES", "MUTATION",
    "NATURAL", "NOT", "NULL", "NULLS", "OFFSET", "ON", "OPTIMIZE", "OPTION", "OR",
    "ORDER", "OUTER", "OVER", "OVERRIDE", "PART", "PARTITION", "PERMANENTLY",
    "PIPELINE", "PLAN", "POLICY", "POPULATE", "PRECEDING", "PREWHERE", "PRIMARY",
    "PRIVILEGES", "PROCESSLIST", "PROFILE", "PROJECTION", "QUALIFY", "QUERY", "QUOTA",
    "RANGE", "RECURSIVE", "RELOAD", "RENAME", "REPLACE", "REPLICA", "REPLICAS",
    "REPLICATED", "RESET", "RESPECT", "RESTORE", "REVOKE", "RIGHT", "ROLE", "ROLLBACK",
    "ROLLUP", "ROW", "ROWS", "SAMPLE", "SELECT", "SEMI", "SENDING", "SENDS", "SET",
    "SETS", "SETTING", "SETTINGS", "SHOW", "SKIP", "SOURCE", "START", "STEP", "STOP",
    "SYNC", "SYNTAX", "SYSTEM", "TABLE", "TABLES", "TEMPORARY", "TEST", "THEN", "TIES",
    "TO", "TOTALS", "TRANSACTION", "TREE", "TRUE", "TRUNCATE", "TTL", "TYPE",
    "UNBOUNDED", "UNCOMPRESSED", "UNDROP", "UNFREEZE", "UNION", "UPDATE", "USE",
    "USER", "USING", "VALUES", "VIEW", "WHEN", "WHERE", "WINDOW", "WITH",
};
