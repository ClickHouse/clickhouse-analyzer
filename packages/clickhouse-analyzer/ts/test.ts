import { describe, it, before } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { SyntaxKind } from "./syntax-kind.js";
import { buildParseResult, SyntaxNode } from "./parse.js";
import {
    extractTableNames,
    extractColumnReferences,
    extractFunctionCalls,
    getStatementType,
} from "./helpers.js";
import type { RawParseResult } from "./types.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

// We can't use the init() wrapper in Node (it uses fetch), so we load WASM
// manually with initSync and call parse_sql directly.
let parseSql: (sql: string) => string;

function parse(sql: string) {
    const raw: RawParseResult = JSON.parse(parseSql(sql));
    return buildParseResult(raw);
}

before(async () => {
    const wasmBytes = readFileSync(join(root, "pkg", "clickhouse_analyzer_bg.wasm"));
    const pkg = await import(join(root, "pkg", "clickhouse_analyzer.js"));
    pkg.initSync({ module: new WebAssembly.Module(wasmBytes) });
    parseSql = pkg.parse_sql;
});

// ---------------------------------------------------------------------------
// Basic parsing
// ---------------------------------------------------------------------------

describe("parse", () => {
    it("parses valid SQL without errors", () => {
        const result = parse("SELECT 1");
        assert.equal(result.errors.length, 0);
        assert.equal(result.ok, true);
        assert.equal(result.tree.kind, SyntaxKind.File);
    });

    it("reports errors for invalid SQL", () => {
        const result = parse("SELECT (1 +");
        assert.ok(result.errors.length > 0);
        assert.equal(result.ok, false);
    });

    it("preserves source text", () => {
        const sql = "SELECT a, b FROM t";
        const result = parse(sql);
        assert.equal(result.source, sql);
    });

    it("parses empty input", () => {
        const result = parse("");
        assert.equal(result.errors.length, 0);
        assert.equal(result.tree.kind, SyntaxKind.File);
    });

    it("parses multiple statements", () => {
        const result = parse("SELECT 1; SELECT 2");
        assert.equal(result.errors.length, 0);
    });
});

// ---------------------------------------------------------------------------
// SyntaxNode navigation
// ---------------------------------------------------------------------------

describe("SyntaxNode", () => {
    it("text() returns source text for a node", () => {
        const result = parse("SELECT 42");
        const num = result.findFirst(SyntaxKind.NumberLiteral);
        assert.ok(num);
        assert.equal(num.text(), "42");
    });

    it("findAll returns all matching descendants", () => {
        const result = parse("SELECT a, b, c FROM t");
        const refs = result.findAll(SyntaxKind.ColumnReference);
        assert.equal(refs.length, 3);
        assert.deepEqual(
            refs.map((r) => r.text().trim()),
            ["a", "b", "c"],
        );
    });

    it("findFirst returns first match or null", () => {
        const result = parse("SELECT 1 FROM t WHERE x > 0");
        const where = result.findFirst(SyntaxKind.WhereClause);
        assert.ok(where);
        assert.ok(where.text().startsWith("WHERE"));

        const join = result.findFirst(SyntaxKind.JoinClause);
        assert.equal(join, null);
    });

    it("children includes tokens and subtrees", () => {
        const result = parse("SELECT 1");
        const select = result.findFirst(SyntaxKind.SelectClause);
        assert.ok(select);
        assert.ok(select.children.length > 0);
    });

    it("treeChildren filters to subtree children only", () => {
        const result = parse("SELECT a, b FROM t");
        const select = result.findFirst(SyntaxKind.SelectClause);
        assert.ok(select);
        const trees = select.treeChildren();
        assert.ok(trees.every((c) => !c.isToken));
    });

    it("tokenChildren filters to token children only", () => {
        const result = parse("SELECT a FROM t");
        const select = result.findFirst(SyntaxKind.SelectClause);
        assert.ok(select);
        const tokens = select.tokenChildren();
        assert.ok(tokens.every((c) => c.isToken));
    });

    it("parent references are set correctly", () => {
        const result = parse("SELECT a FROM t");
        const colRef = result.findFirst(SyntaxKind.ColumnReference);
        assert.ok(colRef);
        assert.ok(colRef.parent);
        assert.equal(colRef.parent.kind, SyntaxKind.ColumnList);
    });

    it("ancestors() walks up to root", () => {
        const result = parse("SELECT a FROM t");
        const colRef = result.findFirst(SyntaxKind.ColumnReference);
        assert.ok(colRef);
        const ancestors = colRef.ancestors();
        assert.ok(ancestors.length >= 2);
        assert.equal(ancestors[ancestors.length - 1].kind, SyntaxKind.File);
    });

    it("walk visits all nodes depth-first", () => {
        const result = parse("SELECT 1");
        const kinds: string[] = [];
        result.tree.walk((node) => kinds.push(node.kind));
        assert.ok(kinds.length > 0);
        assert.equal(kinds[0], SyntaxKind.File);
    });

    it("walk reports correct depth", () => {
        const result = parse("SELECT 1");
        const depths: number[] = [];
        result.tree.walk((_node, depth) => depths.push(depth));
        assert.equal(depths[0], 0); // root
        assert.ok(depths.some((d) => d > 0)); // has children
    });

    it("isToken distinguishes tokens from trees", () => {
        const result = parse("SELECT 1");
        const tokens: SyntaxNode[] = [];
        const trees: SyntaxNode[] = [];
        result.tree.walk((node) => {
            if (node.isToken) tokens.push(node);
            else trees.push(node);
        });
        assert.ok(tokens.length > 0);
        assert.ok(trees.length > 0);
    });

    it("start/end byte offsets are correct", () => {
        const sql = "SELECT abc FROM t";
        const result = parse(sql);
        const colRef = result.findFirst(SyntaxKind.ColumnReference);
        assert.ok(colRef);
        // CST nodes may include trailing whitespace in their span
        assert.ok(sql.slice(colRef.start, colRef.end).startsWith("abc"));
    });
});

// ---------------------------------------------------------------------------
// extractTableNames
// ---------------------------------------------------------------------------

describe("extractTableNames", () => {
    it("extracts simple table name", () => {
        const result = parse("SELECT 1 FROM users");
        assert.deepEqual(extractTableNames(result), ["users"]);
    });

    it("extracts qualified table name", () => {
        const result = parse("SELECT 1 FROM db.users");
        assert.deepEqual(extractTableNames(result), ["db.users"]);
    });

    it("extracts multiple tables from JOIN", () => {
        const result = parse("SELECT 1 FROM a JOIN b ON a.id = b.id");
        const tables = extractTableNames(result);
        assert.ok(tables.includes("a"));
        assert.ok(tables.includes("b"));
    });

    it("extracts tables from subquery", () => {
        const result = parse("SELECT * FROM (SELECT 1 FROM inner_t)");
        const tables = extractTableNames(result);
        assert.ok(tables.includes("inner_t"));
    });

    it("returns empty for no tables", () => {
        const result = parse("SELECT 1");
        assert.deepEqual(extractTableNames(result), []);
    });
});

// ---------------------------------------------------------------------------
// extractColumnReferences
// ---------------------------------------------------------------------------

describe("extractColumnReferences", () => {
    it("extracts column names from SELECT", () => {
        const result = parse("SELECT a, b FROM t");
        const cols = extractColumnReferences(result);
        assert.ok(cols.includes("a"));
        assert.ok(cols.includes("b"));
    });

    it("extracts columns from WHERE", () => {
        const result = parse("SELECT 1 FROM t WHERE x > 0 AND y = 1");
        const cols = extractColumnReferences(result);
        assert.ok(cols.includes("x"));
        assert.ok(cols.includes("y"));
    });
});

// ---------------------------------------------------------------------------
// extractFunctionCalls
// ---------------------------------------------------------------------------

describe("extractFunctionCalls", () => {
    it("extracts function name and args", () => {
        const result = parse("SELECT count(*) FROM t");
        const fns = extractFunctionCalls(result);
        assert.ok(fns.length >= 1);
        assert.equal(fns[0].name, "count");
    });

    it("extracts multiple functions", () => {
        const result = parse("SELECT sum(a), avg(b) FROM t");
        const fns = extractFunctionCalls(result);
        const names = fns.map((f) => f.name);
        assert.ok(names.includes("sum"));
        assert.ok(names.includes("avg"));
    });

    it("returns empty for no functions", () => {
        const result = parse("SELECT a FROM t");
        const fns = extractFunctionCalls(result);
        assert.equal(fns.length, 0);
    });
});

// ---------------------------------------------------------------------------
// getStatementType
// ---------------------------------------------------------------------------

describe("getStatementType", () => {
    const cases: [string, string][] = [
        ["SELECT 1", "SELECT"],
        ["INSERT INTO t VALUES (1)", "INSERT"],
        ["CREATE TABLE t (a Int32) ENGINE = Memory", "CREATE"],
        ["DROP TABLE t", "DROP"],
        ["ALTER TABLE t ADD COLUMN b String", "ALTER"],
        ["SHOW TABLES", "SHOW"],
        ["USE db", "USE"],
        ["SET max_threads = 1", "SET"],
        ["TRUNCATE TABLE t", "TRUNCATE"],
        ["OPTIMIZE TABLE t", "OPTIMIZE"],
        ["EXPLAIN SELECT 1", "EXPLAIN"],
        ["DESCRIBE TABLE t", "DESCRIBE"],
    ];

    for (const [sql, expected] of cases) {
        it(`returns "${expected}" for: ${sql}`, () => {
            const result = parse(sql);
            assert.equal(getStatementType(result), expected);
        });
    }
});

// ---------------------------------------------------------------------------
// ClickHouse-specific SQL
// ---------------------------------------------------------------------------

describe("ClickHouse-specific", () => {
    it("parses FROM before SELECT", () => {
        const result = parse("FROM t SELECT a");
        assert.equal(result.ok, true);
        assert.deepEqual(extractTableNames(result), ["t"]);
    });

    it("parses PREWHERE", () => {
        const result = parse("SELECT a FROM t PREWHERE x > 0");
        const pw = result.findFirst(SyntaxKind.PrewhereClause);
        assert.ok(pw);
    });

    it("parses SETTINGS clause", () => {
        const result = parse("SELECT 1 SETTINGS max_threads = 1");
        const settings = result.findFirst(SyntaxKind.SettingsClause);
        assert.ok(settings);
    });

    it("parses ARRAY JOIN", () => {
        const result = parse("SELECT x FROM t ARRAY JOIN arr AS x");
        const aj = result.findFirst(SyntaxKind.ArrayJoinClause);
        assert.ok(aj);
    });

    it("parses LIMIT BY", () => {
        const result = parse("SELECT a FROM t LIMIT 1 BY a");
        const lb = result.findFirst(SyntaxKind.LimitByClause);
        assert.ok(lb);
    });
});
