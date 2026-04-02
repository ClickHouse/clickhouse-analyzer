use std::fmt;

#[derive(Debug, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    BackupStatement,
    RestoreStatement,
    BeginStatement,
    CommitStatement,
    RollbackStatement,

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
    WindowSpec,
    QualifyClause,
    SampleClause,
    WithFillClause,

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
    // Dictionary components
    // =======================================================================
    DictionarySource,
    DictionarySourceType,
    DictionaryLayout,
    DictionaryLayoutType,
    DictionaryLifetime,
    DictionaryRange,
    DictionaryKeyValue,

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
    CreateIndexStatement,

    // =======================================================================
    // Access control statements
    // =======================================================================
    CreateUserStatement,
    CreateRoleStatement,
    CreateQuotaStatement,
    CreateRowPolicyStatement,
    CreateSettingsProfileStatement,
    AlterUserStatement,
    DropAccessEntityStatement,

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
    AlterModifyComment,
    AlterModifyQuery,
    AlterMaterializeProjection,
    AlterMaterializeTtl,
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
    IsDistinctFromExpression,
    LikeExpression,
    TupleExpression,
    ArrayExpression,
    ArrayAccessExpression,
    DotAccessExpression,
    TypedJsonAccessExpression,
    MapExpression,
    QueryParameterExpression,
    SubqueryExpression,
    LambdaExpression,
    IntervalExpression,
    WindowExpression,
    ExistsExpression,
    TernaryExpression,
    QualifiedAsterisk,
    NullsModifier,
    FilterClause,
    ColumnTransformer,
    GroupingSetsClause,
    GroupingSet,
    WithTotalsClause,

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
    // Token kinds
    // =======================================================================

    // Trivia
    Whitespace,
    Comment,

    // Identifiers and literals
    BareWord,
    Number,
    StringToken,
    QuotedIdentifier,

    // Brackets
    OpeningRoundBracket,
    ClosingRoundBracket,
    OpeningSquareBracket,
    ClosingSquareBracket,
    OpeningCurlyBrace,
    ClosingCurlyBrace,

    // Punctuation
    Comma,
    Semicolon,
    VerticalDelimiter,
    Dot,

    // Operators and special symbols
    Star,
    HereDoc,
    DollarSign,
    Plus,
    Minus,
    Slash,
    Percent,
    Arrow,
    QuestionMark,
    Colon,
    Caret,
    DoubleColon,
    Equals,
    NotEquals,
    Less,
    Greater,
    LessOrEquals,
    GreaterOrEquals,
    Spaceship,
    PipeMark,
    Concatenation,

    // MySQL-style variables
    At,
    DoubleAt,

    // End of stream
    EndOfStream,

    // Error tokens
    ErrorToken,
    ErrorMultilineCommentIsNotClosed,
    ErrorSingleQuoteIsNotClosed,
    ErrorDoubleQuoteIsNotClosed,
    ErrorBackQuoteIsNotClosed,
    ErrorSingleExclamationMark,
    ErrorSinglePipeMark,
    ErrorWrongNumber,
    ErrorMaxQuerySizeExceeded,
}

impl fmt::Display for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyntaxKind::BareWord => write!(f, "identifier or keyword"),
            SyntaxKind::Number => write!(f, "number"),
            SyntaxKind::StringToken => write!(f, "string literal"),
            SyntaxKind::QuotedIdentifier => write!(f, "quoted identifier"),
            SyntaxKind::OpeningRoundBracket => write!(f, "("),
            SyntaxKind::ClosingRoundBracket => write!(f, ")"),
            _ => write!(f, "{:?}", self),
        }
    }
}
