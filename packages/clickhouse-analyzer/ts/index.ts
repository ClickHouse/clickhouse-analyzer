export { SyntaxKind } from "./syntax-kind.js";
export type { SyntaxKind as SyntaxKindType } from "./syntax-kind.js";
export {
    SyntaxNode,
    ParseResult,
    SyntaxError,
    buildParseResult,
} from "./parse.js";
export {
    extractTableNames,
    extractColumnReferences,
    extractFunctionCalls,
    getStatementType,
} from "./helpers.js";
export type {
    RawToken,
    RawTree,
    RawChild,
    RawSyntaxError,
    RawParseResult,
} from "./types.js";

import initWasm, {
    parse_sql as wasmParseSql,
    format_sql as wasmFormatSql,
    get_tree as wasmGetTree,
    get_diagnostics as wasmGetDiagnostics,
} from "../pkg/clickhouse_analyzer.js";
import type { RawParseResult } from "./types.js";
import { buildParseResult, type ParseResult } from "./parse.js";

const MAX_INPUT_SIZE = 1_048_576; // 1MB

function checkInputSize(sql: string): void {
    if (sql.length > MAX_INPUT_SIZE) {
        throw new Error(`Input exceeds maximum size of ${MAX_INPUT_SIZE} bytes`);
    }
}

let initialized = false;

/**
 * Initialize the WASM module. Must be called before any other function.
 * Can be called with a custom URL/path to the .wasm file, or will use
 * the default co-located file.
 */
export async function init(
    wasmInput?: Parameters<typeof initWasm>[0],
): Promise<void> {
    if (initialized) return;
    await initWasm(wasmInput);
    initialized = true;
}

function ensureInit(): void {
    if (!initialized) {
        throw new Error(
            "clickhouse-analyzer WASM not initialized. Call init() first.",
        );
    }
}

/**
 * Parse a ClickHouse SQL string and return a structured ParseResult
 * with tree navigation capabilities.
 */
export function parse(sql: string): ParseResult {
    ensureInit();
    checkInputSize(sql);
    const json = wasmParseSql(sql);
    let raw: RawParseResult;
    try {
        raw = JSON.parse(json);
    } catch (e) {
        throw new Error(`Failed to parse WASM output as JSON: ${e instanceof Error ? e.message : String(e)}`);
    }
    return buildParseResult(raw);
}

/**
 * Format a ClickHouse SQL string.
 */
export function format(sql: string): string {
    ensureInit();
    checkInputSize(sql);
    return wasmFormatSql(sql);
}

/**
 * Get a debug tree representation of parsed SQL.
 */
export function getTree(sql: string): string {
    ensureInit();
    checkInputSize(sql);
    return wasmGetTree(sql);
}

/**
 * Get diagnostics (errors, warnings) as a JSON string.
 */
export function getDiagnostics(sql: string): string {
    ensureInit();
    checkInputSize(sql);
    return wasmGetDiagnostics(sql);
}
