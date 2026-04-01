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

    // =======================================================================
    // Statements
    // =======================================================================
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
    ExplainStatement,
    DescribeStatement,
    ExistsStatement,
    CheckStatement,
    KillStatement,
    GrantStatement,
    RevokeStatement,
    AttachStatement,
    DetachStatement,
    ExchangeStatement,
    UndropStatement,

    // =======================================================================
    // SELECT clauses
    // =======================================================================
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
    FormatClause,
    UnionClause,
    WindowClause,
    WindowDefinition,
    WindowFrame,

    // =======================================================================
    // CREATE TABLE components
    // =======================================================================
    TableDefinition,
    DatabaseDefinition,
    ViewDefinition,
    MaterializedViewDefinition,
    DictionaryDefinition,
    FunctionDefinition,
    EngineClause,
    OrderByDefinition,
    PartitionByDefinition,
    PrimaryKeyDefinition,
    SampleByDefinition,
    TtlDefinition,
    OnClusterClause,
    IfNotExistsClause,
    IfExistsClause,
    AsClause,

    // =======================================================================
    // Column definitions
    // =======================================================================
    ColumnDefinition,
    ColumnDefinitionList,
    ColumnTypeDefinition,
    ColumnConstraint,
    ColumnDefault,
    ColumnCodec,
    ColumnTtl,
    ColumnComment,
    TableConstraint,

    // =======================================================================
    // Index / Projection / Constraint definitions
    // =======================================================================
    IndexDefinition,
    ProjectionDefinition,
    ConstraintDefinition,

    // =======================================================================
    // Table components
    // =======================================================================
    TableIdentifier,
    TableExpression,
    TableFunction,

    // =======================================================================
    // ALTER commands
    // =======================================================================
    AlterCommandList,
    AlterAddColumn,
    AlterDropColumn,
    AlterModifyColumn,
    AlterRenameColumn,
    AlterClearColumn,
    AlterCommentColumn,
    AlterAddIndex,
    AlterDropIndex,
    AlterClearIndex,
    AlterMaterializeIndex,
    AlterAddProjection,
    AlterDropProjection,
    AlterAddConstraint,
    AlterDropConstraint,
    AlterModifyOrderBy,
    AlterModifyTtl,
    AlterModifySetting,
    AlterResetSetting,
    AlterDropPartition,
    AlterAttachPartition,
    AlterDetachPartition,
    AlterFreezePartition,
    AlterDeleteWhere,
    AlterUpdateWhere,

    // =======================================================================
    // INSERT components
    // =======================================================================
    InsertColumnsClause,
    InsertValuesClause,
    InsertFormatClause,
    ValueRow,

    // =======================================================================
    // Expressions
    // =======================================================================
    Expression,
    Asterisk,
    Identifier,
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
    DotAccessExpression,
    MapExpression,
    QueryParameterExpression,
    SubqueryExpression,
    LambdaExpression,
    IntervalExpression,
    WindowExpression,
    ExistsExpression,

    // =======================================================================
    // Literals
    // =======================================================================
    NumberLiteral,
    StringLiteral,
    DateLiteral,
    BooleanLiteral,
    NullLiteral,

    // =======================================================================
    // Lists
    // =======================================================================
    ColumnList,
    ExpressionList,
    OrderByList,
    GroupByList,
    SettingList,
    IdentifierList,

    // =======================================================================
    // Join
    // =======================================================================
    JoinType,
    JoinConstraint,

    // =======================================================================
    // CASE components
    // =======================================================================
    WhenClause,

    // =======================================================================
    // Compound items
    // =======================================================================
    OrderByItem,
    SettingItem,
    WithExpressionItem,
    TableAlias,
    UsingList,
    RenameItem,

    // =======================================================================
    // Data types
    // =======================================================================
    DataType,
    DataTypeParameters,
    NestedDataType,
    EnumValue,

    // =======================================================================
    // ClickHouse-specific
    // =======================================================================
    PartitionExpression,
    SampleExpression,

    // =======================================================================
    // EXPLAIN components
    // =======================================================================
    ExplainKind,

    // =======================================================================
    // SHOW components
    // =======================================================================
    ShowTarget,
    LikeClause,
    FromDatabaseClause,

    // =======================================================================
    // SYSTEM components
    // =======================================================================
    SystemCommand,

    // =======================================================================
    // GRANT / REVOKE components
    // =======================================================================
    PrivilegeList,
    Privilege,
    GrantTarget,

    // =======================================================================
    // KILL components
    // =======================================================================
    KillTarget,

    // =======================================================================
    // DELETE / UPDATE components
    // =======================================================================
    SetClause,
    AssignmentList,
    Assignment,

    // =======================================================================
    // Trivia
    // =======================================================================
    Whitespace,
    LineComment,
    BlockComment,
}
