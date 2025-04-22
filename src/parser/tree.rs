use std::fmt;
use crate::lexer::token::{Token, TokenKind};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum TreeKind {
    // Error handling
    ErrorTree,

    // Root and container nodes
    File,      // Root of the tree
    QueryList, // Multiple queries separated by semicolons

    // Statements
    SelectStatement,
    InsertStatement,
    UpdateStatement,
    DeleteStatement,
    CreateStatement,
    AlterStatement,
    DropStatement,
    TruncateStatement,
    RenameStatement,
    ShowStatement,
    UseStatement,
    SetStatement,
    OptimizeStatement,
    SystemStatement,

    // SELECT statement components
    WithClause,      // WITH subqueries
    SelectClause,    // SELECT columns
    FromClause,      // FROM tables
    JoinClause,      // Any join
    ArrayJoinClause, // ARRAY JOIN
    PrewhereClause,  // PREWHERE (ClickHouse specific)
    WhereClause,     // WHERE conditions
    GroupByClause,   // GROUP BY columns
    HavingClause,    // HAVING conditions
    OrderByClause,   // ORDER BY columns
    LimitByClause,   // LIMIT BY (ClickHouse specific)
    LimitClause,     // LIMIT rows
    SettingsClause,  // SETTINGS (ClickHouse specific)

    // CREATE statement components
    TableDefinition,
    DatabaseDefinition,
    ViewDefinition,
    MaterializedViewDefinition,
    DictionaryDefinition,

    // Column definition components
    ColumnDefinition,
    ColumnTypeDefinition,
    ColumnConstraint,
    TableConstraint,

    // Table components
    TableIdentifier, // Database.Table
    TableExpression, // Table or subquery
    TableFunction,   // Table function like merge()

    // Expressions
    Expression,         // Generic expression
    Asterisk,           // Asterisk expression
    ColumnReference,    // Column reference
    ColumnAlias,        // Column alias
    QualifiedName,      // Database.table.column
    FunctionCall,       // Function call
    AggregateFunction,  // Aggregate function with possible DISTINCT/ALL
    CastExpression,     // CAST(x AS type)
    CaseExpression,     // CASE WHEN ... THEN
    BinaryExpression,   // a + b, a AND b, etc.
    UnaryExpression,    // NOT a, -b
    BetweenExpression,  // a BETWEEN b AND c
    InExpression,       // a IN (b, c)
    TupleExpression,    // (a, b, c)
    ArrayExpression,    // [a, b, c]
    MapExpression,      // {a:b, c:d}
    SubqueryExpression, // (SELECT ...)
    LambdaExpression,   // x -> expr

    // Literals
    NumberLiteral,  // 123, 3.14
    StringLiteral,  // 'text'
    DateLiteral,    // DATE '2023-01-01'
    BooleanLiteral, // true, false
    NullLiteral,    // NULL

    // Lists and collections
    ColumnList,     // List of columns
    ExpressionList, // List of expressions
    OrderByList,    // List of ORDER BY items
    GroupByList,    // List of GROUP BY items
    SettingList,    // List of settings

    // Join specific
    JoinType,       // LEFT, INNER, etc.
    JoinConstraint, // ON clause or USING clause

    // Items that are part of larger constructs
    OrderByItem,        // column ASC NULLS FIRST
    WithExpressionItem, // name AS (subquery)

    // Data type definitions
    DataType, // Int32, String, etc.
    DataTypeParameters,
    NestedDataType, // Array(Int32), Tuple(...)
    EnumValue,      // 'value' = 1

    // ClickHouse specific
    PartitionExpression, // PARTITION BY expr
    SampleExpression,    // SAMPLE n

    // Trivia
    Whitespace,
    LineComment,  // -- comment
    BlockComment, // /* comment */
}

pub struct Tree {
    pub kind: TreeKind,
    pub children: Vec<Child>,
}

pub enum Child {
    Token(Token),
    Tree(Tree),
}

impl Child {
    pub fn is_token(&self) -> bool {
        matches!(self, Child::Token(_))
    }
    
    pub fn is_tree(&self) -> bool {
        matches!(self, Child::Token(_))
    }

    pub fn get_token_with_kind(&self, kind: TokenKind) -> Option<&Token> {
        match self {
            Child::Token(token) if token.kind == kind => Some(token),
            _ => None,
        }
    }
    
    pub fn get_tree_with_kind(&self, kind: TreeKind) -> Option<&Tree> {
        match self {
            Child::Tree(tree) if tree.kind == kind => Some(tree),
            _ => None,
        }
    }
}

pub trait ChildOptionExt {
    fn get_token_with_kind(&self, kind: TokenKind) -> Option<&Token>;
    fn get_tree_with_kind(&self, kind: TreeKind) -> Option<&Tree>;
}

impl ChildOptionExt for Option<&Child> {
    fn get_token_with_kind(&self, kind: TokenKind) -> Option<&Token> {
        match self {
            Some(Child::Token(token)) if token.kind == kind => Some(token),
            _ => None,
        }
    }
    
    fn get_tree_with_kind(&self, kind: TreeKind) -> Option<&Tree> {
        match self {
            Some(Child::Tree(tree)) if tree.kind == kind => Some(tree),
            _ => None
        }
    }
}

#[macro_export]
macro_rules! format_to {
    ($buf:expr) => ();
    ($buf:expr, $lit:literal $($arg:tt)*) => {
        { use ::std::fmt::Write as _; let _ = ::std::write!($buf, $lit $($arg)*); }
    };
}

impl Tree {
    pub fn print(&self, buf: &mut String, level: usize) {
        let indent = "  ".repeat(level);
        format_to!(buf, "{indent}{:?}\n", self.kind);
        for child in &self.children {
            match child {
                Child::Token(token) => {
                    if token.kind == TokenKind::Whitespace {
                        continue;
                    }
                    format_to!(buf, "{indent}  '{}'\n", token.text)
                }
                Child::Tree(tree) => tree.print(buf, level + 1),
            }
        }
        assert!(buf.ends_with('\n'));
    }
}

impl fmt::Debug for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = String::new();
        self.print(&mut buf, 0);
        write!(f, "{}", buf)
    }
}
