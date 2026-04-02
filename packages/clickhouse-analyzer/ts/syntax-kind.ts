/**
 * SyntaxKind enum — mirrors the Rust SyntaxKind enum exactly.
 * Auto-generated from src/parser/syntax_kind.rs
 */
export const SyntaxKind = {
    // Error recovery
    Error: "Error",

    // Root
    File: "File",
    QueryList: "QueryList",

    // Statements
    SelectStatement: "SelectStatement",
    InsertStatement: "InsertStatement",
    UpdateStatement: "UpdateStatement",
    DeleteStatement: "DeleteStatement",
    CreateStatement: "CreateStatement",
    AlterStatement: "AlterStatement",
    DropStatement: "DropStatement",
    TruncateStatement: "TruncateStatement",
    RenameStatement: "RenameStatement",
    ShowStatement: "ShowStatement",
    UseStatement: "UseStatement",
    SetStatement: "SetStatement",
    OptimizeStatement: "OptimizeStatement",
    SystemStatement: "SystemStatement",
    ExplainStatement: "ExplainStatement",
    DescribeStatement: "DescribeStatement",
    ExistsStatement: "ExistsStatement",
    CheckStatement: "CheckStatement",
    KillStatement: "KillStatement",
    GrantStatement: "GrantStatement",
    RevokeStatement: "RevokeStatement",
    AttachStatement: "AttachStatement",
    DetachStatement: "DetachStatement",
    ExchangeStatement: "ExchangeStatement",
    UndropStatement: "UndropStatement",
    BackupStatement: "BackupStatement",
    RestoreStatement: "RestoreStatement",
    BeginStatement: "BeginStatement",
    CommitStatement: "CommitStatement",
    RollbackStatement: "RollbackStatement",

    // SELECT clauses
    WithClause: "WithClause",
    SelectClause: "SelectClause",
    FromClause: "FromClause",
    JoinClause: "JoinClause",
    ArrayJoinClause: "ArrayJoinClause",
    PrewhereClause: "PrewhereClause",
    WhereClause: "WhereClause",
    GroupByClause: "GroupByClause",
    HavingClause: "HavingClause",
    OrderByClause: "OrderByClause",
    LimitByClause: "LimitByClause",
    LimitClause: "LimitClause",
    SettingsClause: "SettingsClause",
    FormatClause: "FormatClause",
    UnionClause: "UnionClause",
    WindowClause: "WindowClause",
    WindowDefinition: "WindowDefinition",
    WindowFrame: "WindowFrame",
    WindowSpec: "WindowSpec",
    QualifyClause: "QualifyClause",
    SampleClause: "SampleClause",
    WithFillClause: "WithFillClause",

    // CREATE TABLE components
    TableDefinition: "TableDefinition",
    DatabaseDefinition: "DatabaseDefinition",
    ViewDefinition: "ViewDefinition",
    MaterializedViewDefinition: "MaterializedViewDefinition",
    DictionaryDefinition: "DictionaryDefinition",
    FunctionDefinition: "FunctionDefinition",
    EngineClause: "EngineClause",
    OrderByDefinition: "OrderByDefinition",
    PartitionByDefinition: "PartitionByDefinition",
    PrimaryKeyDefinition: "PrimaryKeyDefinition",
    SampleByDefinition: "SampleByDefinition",
    TtlDefinition: "TtlDefinition",
    OnClusterClause: "OnClusterClause",
    IfNotExistsClause: "IfNotExistsClause",
    IfExistsClause: "IfExistsClause",
    AsClause: "AsClause",

    // Dictionary components
    DictionarySource: "DictionarySource",
    DictionarySourceType: "DictionarySourceType",
    DictionaryLayout: "DictionaryLayout",
    DictionaryLayoutType: "DictionaryLayoutType",
    DictionaryLifetime: "DictionaryLifetime",
    DictionaryRange: "DictionaryRange",
    DictionaryKeyValue: "DictionaryKeyValue",

    // Column definitions
    ColumnDefinition: "ColumnDefinition",
    ColumnDefinitionList: "ColumnDefinitionList",
    ColumnTypeDefinition: "ColumnTypeDefinition",
    ColumnConstraint: "ColumnConstraint",
    ColumnDefault: "ColumnDefault",
    ColumnCodec: "ColumnCodec",
    ColumnTtl: "ColumnTtl",
    ColumnComment: "ColumnComment",
    TableConstraint: "TableConstraint",

    // Index / Projection / Constraint
    IndexDefinition: "IndexDefinition",
    ProjectionDefinition: "ProjectionDefinition",
    ConstraintDefinition: "ConstraintDefinition",
    CreateIndexStatement: "CreateIndexStatement",

    // Access control
    CreateUserStatement: "CreateUserStatement",
    CreateRoleStatement: "CreateRoleStatement",
    CreateQuotaStatement: "CreateQuotaStatement",
    CreateRowPolicyStatement: "CreateRowPolicyStatement",
    CreateSettingsProfileStatement: "CreateSettingsProfileStatement",
    AlterUserStatement: "AlterUserStatement",
    DropAccessEntityStatement: "DropAccessEntityStatement",

    // Table components
    TableIdentifier: "TableIdentifier",
    TableExpression: "TableExpression",
    TableFunction: "TableFunction",

    // ALTER commands
    AlterCommandList: "AlterCommandList",
    AlterAddColumn: "AlterAddColumn",
    AlterDropColumn: "AlterDropColumn",
    AlterModifyColumn: "AlterModifyColumn",
    AlterRenameColumn: "AlterRenameColumn",
    AlterClearColumn: "AlterClearColumn",
    AlterCommentColumn: "AlterCommentColumn",
    AlterAddIndex: "AlterAddIndex",
    AlterDropIndex: "AlterDropIndex",
    AlterClearIndex: "AlterClearIndex",
    AlterMaterializeIndex: "AlterMaterializeIndex",
    AlterAddProjection: "AlterAddProjection",
    AlterDropProjection: "AlterDropProjection",
    AlterAddConstraint: "AlterAddConstraint",
    AlterDropConstraint: "AlterDropConstraint",
    AlterModifyOrderBy: "AlterModifyOrderBy",
    AlterModifyTtl: "AlterModifyTtl",
    AlterModifySetting: "AlterModifySetting",
    AlterModifyComment: "AlterModifyComment",
    AlterModifyQuery: "AlterModifyQuery",
    AlterMaterializeProjection: "AlterMaterializeProjection",
    AlterMaterializeTtl: "AlterMaterializeTtl",
    AlterResetSetting: "AlterResetSetting",
    AlterDropPartition: "AlterDropPartition",
    AlterAttachPartition: "AlterAttachPartition",
    AlterDetachPartition: "AlterDetachPartition",
    AlterFreezePartition: "AlterFreezePartition",
    AlterDeleteWhere: "AlterDeleteWhere",
    AlterUpdateWhere: "AlterUpdateWhere",

    // INSERT components
    InsertColumnsClause: "InsertColumnsClause",
    InsertValuesClause: "InsertValuesClause",
    InsertFormatClause: "InsertFormatClause",
    ValueRow: "ValueRow",

    // Expressions
    Expression: "Expression",
    Asterisk: "Asterisk",
    Identifier: "Identifier",
    ColumnReference: "ColumnReference",
    ColumnAlias: "ColumnAlias",
    QualifiedName: "QualifiedName",
    FunctionCall: "FunctionCall",
    AggregateFunction: "AggregateFunction",
    CastExpression: "CastExpression",
    CaseExpression: "CaseExpression",
    BinaryExpression: "BinaryExpression",
    UnaryExpression: "UnaryExpression",
    BetweenExpression: "BetweenExpression",
    InExpression: "InExpression",
    IsNullExpression: "IsNullExpression",
    IsDistinctFromExpression: "IsDistinctFromExpression",
    LikeExpression: "LikeExpression",
    TupleExpression: "TupleExpression",
    ArrayExpression: "ArrayExpression",
    ArrayAccessExpression: "ArrayAccessExpression",
    DotAccessExpression: "DotAccessExpression",
    TypedJsonAccessExpression: "TypedJsonAccessExpression",
    MapExpression: "MapExpression",
    QueryParameterExpression: "QueryParameterExpression",
    SubqueryExpression: "SubqueryExpression",
    LambdaExpression: "LambdaExpression",
    IntervalExpression: "IntervalExpression",
    WindowExpression: "WindowExpression",
    ExistsExpression: "ExistsExpression",
    TernaryExpression: "TernaryExpression",
    QualifiedAsterisk: "QualifiedAsterisk",
    NullsModifier: "NullsModifier",
    FilterClause: "FilterClause",
    ColumnTransformer: "ColumnTransformer",
    GroupingSetsClause: "GroupingSetsClause",
    GroupingSet: "GroupingSet",
    WithTotalsClause: "WithTotalsClause",

    // Literals
    NumberLiteral: "NumberLiteral",
    StringLiteral: "StringLiteral",
    DateLiteral: "DateLiteral",
    BooleanLiteral: "BooleanLiteral",
    NullLiteral: "NullLiteral",

    // Lists
    ColumnList: "ColumnList",
    ExpressionList: "ExpressionList",
    OrderByList: "OrderByList",
    GroupByList: "GroupByList",
    SettingList: "SettingList",
    IdentifierList: "IdentifierList",

    // Join
    JoinType: "JoinType",
    JoinConstraint: "JoinConstraint",

    // CASE components
    WhenClause: "WhenClause",

    // Compound items
    OrderByItem: "OrderByItem",
    SettingItem: "SettingItem",
    WithExpressionItem: "WithExpressionItem",
    TableAlias: "TableAlias",
    UsingList: "UsingList",
    RenameItem: "RenameItem",

    // Data types
    DataType: "DataType",
    DataTypeParameters: "DataTypeParameters",
    NestedDataType: "NestedDataType",
    EnumValue: "EnumValue",

    // ClickHouse-specific
    PartitionExpression: "PartitionExpression",
    SampleExpression: "SampleExpression",

    // EXPLAIN components
    ExplainKind: "ExplainKind",

    // SHOW components
    ShowTarget: "ShowTarget",
    LikeClause: "LikeClause",
    FromDatabaseClause: "FromDatabaseClause",

    // SYSTEM components
    SystemCommand: "SystemCommand",

    // GRANT / REVOKE components
    PrivilegeList: "PrivilegeList",
    Privilege: "Privilege",
    GrantTarget: "GrantTarget",

    // KILL components
    KillTarget: "KillTarget",

    // DELETE / UPDATE components
    SetClause: "SetClause",
    AssignmentList: "AssignmentList",
    Assignment: "Assignment",

    // Token kinds — Trivia
    Whitespace: "Whitespace",
    Comment: "Comment",

    // Token kinds — Identifiers and literals
    BareWord: "BareWord",
    Number: "Number",
    StringToken: "StringToken",
    QuotedIdentifier: "QuotedIdentifier",

    // Token kinds — Brackets
    OpeningRoundBracket: "OpeningRoundBracket",
    ClosingRoundBracket: "ClosingRoundBracket",
    OpeningSquareBracket: "OpeningSquareBracket",
    ClosingSquareBracket: "ClosingSquareBracket",
    OpeningCurlyBrace: "OpeningCurlyBrace",
    ClosingCurlyBrace: "ClosingCurlyBrace",

    // Token kinds — Punctuation
    Comma: "Comma",
    Semicolon: "Semicolon",
    VerticalDelimiter: "VerticalDelimiter",
    Dot: "Dot",

    // Token kinds — Operators
    Star: "Star",
    HereDoc: "HereDoc",
    DollarSign: "DollarSign",
    Plus: "Plus",
    Minus: "Minus",
    Slash: "Slash",
    Percent: "Percent",
    Arrow: "Arrow",
    QuestionMark: "QuestionMark",
    Colon: "Colon",
    Caret: "Caret",
    DoubleColon: "DoubleColon",
    Equals: "Equals",
    NotEquals: "NotEquals",
    Less: "Less",
    Greater: "Greater",
    LessOrEquals: "LessOrEquals",
    GreaterOrEquals: "GreaterOrEquals",
    Spaceship: "Spaceship",
    PipeMark: "PipeMark",
    Concatenation: "Concatenation",

    // MySQL-style variables
    At: "At",
    DoubleAt: "DoubleAt",

    // End of stream
    EndOfStream: "EndOfStream",

    // Error tokens
    ErrorToken: "ErrorToken",
    ErrorMultilineCommentIsNotClosed: "ErrorMultilineCommentIsNotClosed",
    ErrorSingleQuoteIsNotClosed: "ErrorSingleQuoteIsNotClosed",
    ErrorDoubleQuoteIsNotClosed: "ErrorDoubleQuoteIsNotClosed",
    ErrorBackQuoteIsNotClosed: "ErrorBackQuoteIsNotClosed",
    ErrorSingleExclamationMark: "ErrorSingleExclamationMark",
    ErrorSinglePipeMark: "ErrorSinglePipeMark",
    ErrorWrongNumber: "ErrorWrongNumber",
    ErrorMaxQuerySizeExceeded: "ErrorMaxQuerySizeExceeded",
} as const;

export type SyntaxKind = (typeof SyntaxKind)[keyof typeof SyntaxKind];
