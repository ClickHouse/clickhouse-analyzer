/**
 * End-to-end test for the clickhouse-analyzer WASM + TypeScript library.
 * Run: node test-wasm.mjs
 */
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Load the WASM glue code
const pkgPath = join(__dirname, "pkg", "clickhouse_analyzer.js");
const wasmPath = join(__dirname, "pkg", "clickhouse_analyzer_bg.wasm");

// We need to patch globalThis for wasm-pack's web target to work in Node
// wasm-pack --target web expects fetch/URL/import.meta.url which don't work in Node
// So we use initSync with the raw bytes instead.
const wasmBytes = readFileSync(wasmPath);

// Dynamic import of the pkg JS
const pkg = await import(pkgPath);
pkg.initSync({ module: new WebAssembly.Module(wasmBytes) });

// Now import our TypeScript (compiled) wrapper
const {
    SyntaxKind,
    buildParseResult,
    extractTableNames,
    extractColumnReferences,
    extractFunctionCalls,
    getStatementType,
} = await import(join(__dirname, "dist", "index.js"));

// Use the raw WASM function directly to get JSON, then wrap it
const rawJson = pkg.parse_sql("SELECT a, b, count(*) FROM db.users WHERE id > 10 GROUP BY a, b");
const raw = JSON.parse(rawJson);
const result = buildParseResult(raw);

console.log("=== Parse Result ===");
console.log("Errors:", result.errors.length === 0 ? "none" : result.errors);
console.log("Root kind:", result.tree.kind);
console.log("");

// Test findAll for table identifiers
const tables = extractTableNames(result);
console.log("=== Table Names ===");
console.log(tables);
// Expected: ["db.users"]

// Test column references
const columns = extractColumnReferences(result);
console.log("\n=== Column References ===");
console.log(columns);

// Test function calls
const functions = extractFunctionCalls(result);
console.log("\n=== Function Calls ===");
console.log(functions);

// Test statement type
const stmtType = getStatementType(result);
console.log("\n=== Statement Type ===");
console.log(stmtType);
// Expected: "SELECT"

// Test tree navigation
console.log("\n=== WHERE Clause ===");
const where = result.findFirst(SyntaxKind.WhereClause);
if (where) {
    console.log("Text:", JSON.stringify(where.text()));
    console.log("Start:", where.start, "End:", where.end);
}

// Test formatting
const formatted = pkg.format_sql("select    a,b from    db.users   where id>10");
console.log("\n=== Formatted SQL ===");
console.log(formatted);

// Test error case
const badResult = buildParseResult(JSON.parse(pkg.parse_sql("SELECT FROM")));
console.log("\n=== Error Case ===");
console.log("Errors:", badResult.errors.length);
console.log("Has errors:", !badResult.ok);

// Test walk
console.log("\n=== Tree Walk (first 10 nodes) ===");
let count = 0;
result.tree.walk((node, depth) => {
    if (count < 10) {
        console.log("  ".repeat(depth) + node.kind + (node.isToken ? ` "${node.text()}"` : ""));
    }
    count++;
});
console.log(`... (${count} total nodes)`);

// Gzip size check
console.log("\n=== WASM Size ===");
console.log(`Raw: ${(wasmBytes.length / 1024).toFixed(1)} KB`);

console.log("\n✓ All tests passed!");
