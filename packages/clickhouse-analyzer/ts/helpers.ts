import { SyntaxKind } from "./syntax-kind.js";
import type { ParseResult, SyntaxNode } from "./parse.js";

/**
 * Extract all table names from a parse result.
 * Returns fully qualified names (e.g. "db.users") when a database prefix is present.
 */
export function extractTableNames(result: ParseResult): string[] {
    const tables = result.findAll(SyntaxKind.TableIdentifier);
    return tables.map((node) => {
        // Skip whitespace tokens when assembling the name
        return node.children
            .filter((c) => c.kind !== SyntaxKind.Whitespace)
            .map((c) => c.text())
            .join("");
    });
}

/**
 * Extract all column references from a parse result.
 */
export function extractColumnReferences(result: ParseResult): string[] {
    const refs = result.findAll(SyntaxKind.ColumnReference);
    return refs.map((node) => {
        return node.children
            .filter((c) => c.kind !== SyntaxKind.Whitespace)
            .map((c) => c.text())
            .join("");
    });
}

/**
 * Extract all function calls from a parse result.
 */
export function extractFunctionCalls(
    result: ParseResult,
): { name: string; args: string[] }[] {
    const calls = result.findAll(SyntaxKind.FunctionCall);
    return calls.map((node) => {
        const name = getFunctionName(node);
        const args = getFunctionArgs(node);
        return { name, args };
    });
}

function getFunctionName(funcNode: SyntaxNode): string {
    // The function name is typically the first non-whitespace child (a BareWord or QualifiedName)
    for (const child of funcNode.children) {
        if (child.kind === SyntaxKind.Whitespace) continue;
        if (
            child.kind === SyntaxKind.BareWord ||
            child.kind === SyntaxKind.Identifier ||
            child.kind === SyntaxKind.QualifiedName
        ) {
            return child.text();
        }
        // If we hit a bracket, the name was whatever came before
        if (child.kind === SyntaxKind.OpeningRoundBracket) break;
    }
    return "";
}

function getFunctionArgs(funcNode: SyntaxNode): string[] {
    // Find the ExpressionList inside the function call
    const exprList = funcNode.findFirst(SyntaxKind.ExpressionList);
    if (!exprList) return [];

    // Each direct child that is an expression or column reference is an arg
    return exprList.children
        .filter(
            (c) =>
                c.kind !== SyntaxKind.Whitespace &&
                c.kind !== SyntaxKind.Comma &&
                c.kind !== SyntaxKind.OpeningRoundBracket &&
                c.kind !== SyntaxKind.ClosingRoundBracket,
        )
        .map((c) => c.text().trim());
}

/**
 * Determine the top-level statement type from a parse result.
 * Returns strings like "SELECT", "INSERT", "CREATE TABLE", "ALTER", etc.
 */
export function getStatementType(result: ParseResult): string | null {
    const tree = result.tree;

    // Walk immediate children to find the first statement node
    for (const child of tree.children) {
        if (child.isToken) continue;
        const kind = child.kind;

        if (kind === SyntaxKind.QueryList) {
            // Recurse into QueryList
            for (const stmt of child.children) {
                const type = statementKindToType(stmt.kind);
                if (type) return type;
            }
        }

        const type = statementKindToType(kind);
        if (type) return type;
    }

    return null;
}

const STATEMENT_TYPE_MAP: Record<string, string> = {
    SelectStatement: "SELECT",
    InsertStatement: "INSERT",
    UpdateStatement: "UPDATE",
    DeleteStatement: "DELETE",
    CreateStatement: "CREATE",
    AlterStatement: "ALTER",
    DropStatement: "DROP",
    TruncateStatement: "TRUNCATE",
    RenameStatement: "RENAME",
    ShowStatement: "SHOW",
    UseStatement: "USE",
    SetStatement: "SET",
    OptimizeStatement: "OPTIMIZE",
    SystemStatement: "SYSTEM",
    ExplainStatement: "EXPLAIN",
    DescribeStatement: "DESCRIBE",
    ExistsStatement: "EXISTS",
    CheckStatement: "CHECK",
    KillStatement: "KILL",
    GrantStatement: "GRANT",
    RevokeStatement: "REVOKE",
    AttachStatement: "ATTACH",
    DetachStatement: "DETACH",
    ExchangeStatement: "EXCHANGE",
    UndropStatement: "UNDROP",
    BackupStatement: "BACKUP",
    RestoreStatement: "RESTORE",
    BeginStatement: "BEGIN",
    CommitStatement: "COMMIT",
    RollbackStatement: "ROLLBACK",
};

function statementKindToType(kind: string): string | null {
    return STATEMENT_TYPE_MAP[kind] ?? null;
}
