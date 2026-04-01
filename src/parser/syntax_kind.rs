use serde::Serialize;

#[derive(Debug, PartialEq, Copy, Clone, Serialize)]
#[repr(u16)]
#[allow(dead_code)]
pub enum SyntaxKind {
    // Error recovery
    Error,

    // Root
    File,
    QueryList,

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

    // SELECT clauses
    WithClause,
    SelectClause,
    FromClause,
    JoinClause,
    ArrayJoinClause,
    PrewhereClause,
    WhereClause,
    GroupByClause,
    HavingClause,
    OrderByClause,
    LimitByClause,
    LimitClause,
    SettingsClause,

    // CREATE components
    TableDefinition,
    DatabaseDefinition,
    ViewDefinition,
    MaterializedViewDefinition,
    DictionaryDefinition,

    // Column definitions
    ColumnDefinition,
    ColumnTypeDefinition,
    ColumnConstraint,
    TableConstraint,

    // Table components
    TableIdentifier,
    TableExpression,
    TableFunction,

    // Expressions
    Expression,
    Asterisk,
    ColumnReference,
    ColumnAlias,
    QualifiedName,
    FunctionCall,
    AggregateFunction,
    CastExpression,
    CaseExpression,
    BinaryExpression,
    UnaryExpression,
    BetweenExpression,
    InExpression,
    IsNullExpression,
    LikeExpression,
    TupleExpression,
    ArrayExpression,
    ArrayAccessExpression,
    MapExpression,
    SubqueryExpression,
    LambdaExpression,
    IntervalExpression,

    // Literals
    NumberLiteral,
    StringLiteral,
    DateLiteral,
    BooleanLiteral,
    NullLiteral,

    // Lists
    ColumnList,
    ExpressionList,
    OrderByList,
    GroupByList,
    SettingList,

    // Join
    JoinType,
    JoinConstraint,

    // CASE components
    WhenClause,

    // Compound items
    OrderByItem,
    SettingItem,
    WithExpressionItem,
    TableAlias,
    UsingList,

    // Data types
    DataType,
    DataTypeParameters,
    NestedDataType,
    EnumValue,

    // ClickHouse-specific
    PartitionExpression,
    SampleExpression,

    // Trivia
    Whitespace,
    LineComment,
    BlockComment,
}
