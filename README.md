# clickhouse-analyzer

A resilient parser and tooling backend for ClickHouse SQL.

> Note that this project is experimental. It only covers ~95% of ClickHouse SQL. The cli/WASM interfaces are subject to change.

## CLI

Parse and format ClickHouse SQL from the command line.

```
cargo build --release
```

### Parse (dump CST)

```
$ echo 'SELECT a, count(*) FROM events WHERE ts > now() GROUP BY a' | ./target/release/clickhouse-analyzer
File
  SelectStatement
    SelectClause
      "SELECT" "a" "," "count" "(" "*" ")" ...
    FromClause
      "FROM" TableIdentifier "events"
    WhereClause
      "WHERE" ...
    GroupByClause
      "GROUP" "BY" "a"
```

### Format

```
$ echo 'select a,b from t where x>1 order by a' | ./target/release/clickhouse-analyzer --format
SELECT
    a,
    b
FROM t
WHERE x > 1
ORDER BY a
```

### Diagnostics

Errors and warnings are printed to stderr:

```
$ echo 'SELECT (1 + FROM t' | ./target/release/clickhouse-analyzer
error at 1:12: expected )
error at 1:19: Expected table reference
```

## Project Structure

```
├── src/                              Rust crate (parser, formatter, diagnostics)
├── packages/
│   ├── clickhouse-analyzer/          @clickhouse/analyzer — WASM + TypeScript library
│   └── vscode/                       @clickhouse/analyzer-vscode — VS Code extension
├── tests/                            Integration & property tests
└── benches/                          Benchmarks
```

## LSP Server

Real-time diagnostics, formatting, and semantic highlighting for any LSP-compatible editor.

```
cargo build --release --features lsp
```

This produces `target/release/clickhouse-lsp`. Point any LSP client at it over stdio.

### VS Code Extension

```
cd packages/vscode
npm install
npm run build
code --extensionDevelopmentPath="$(pwd)"
```

Or set the binary path in VS Code settings:

```json
{
  "clickhouse-analyzer.serverPath": "/path/to/clickhouse-lsp"
}
```

### LSP Capabilities

- **Diagnostics** — parse errors as squiggly underlines with enriched messages (bracket matching, clause context)
- **Formatting** — format document via the built-in formatter
- **Semantic highlighting** — context-aware token coloring (keywords, functions, columns, types, tables, operators, strings, numbers, comments, parameters)

## TypeScript / WASM

The parser is available as a WASM-powered TypeScript package (`@clickhouse/analyzer`) for browser and Node.js.

### Build

```
cd packages/clickhouse-analyzer
npm install
npm run build
```

### Usage

```typescript
import { init, parse, format, SyntaxKind } from '@clickhouse/analyzer';

await init();

const result = parse('SELECT a, b, count(*) FROM db.users WHERE id > 10');

result.ok;                                       // true
result.errors;                                   // []
result.findAll(SyntaxKind.TableIdentifier);      // nodes for "db.users"
result.findFirst(SyntaxKind.WhereClause);        // WHERE clause node

// Every node has:
node.kind;          // SyntaxKind string
node.text();        // source text this node spans
node.children;      // child SyntaxNode[]
node.parent;        // parent SyntaxNode | null
node.findAll(kind); // search descendants
node.walk((n, depth) => { ... });

format('select a,b from t where x>1');
// → "SELECT\n    a,\n    b\nFROM t\nWHERE x > 1\n"
```

### Extraction helpers

```typescript
import {
    init, parse,
    extractTableNames,
    extractColumnReferences,
    extractFunctionCalls,
    getStatementType,
} from '@clickhouse/analyzer';

await init();
const result = parse('SELECT count(*), sum(x) FROM db.events WHERE ts > now()');

extractTableNames(result);        // ["db.events"]
extractColumnReferences(result);  // ["x", "ts"]
extractFunctionCalls(result);     // [{ name: "count", args: ["*"] }, ...]
getStatementType(result);         // "SELECT"
```

### Extracting ORDER BY and PRIMARY KEY from CREATE TABLE

```typescript
import { init, parse, SyntaxKind } from '@clickhouse/analyzer';

await init();
const result = parse(`
  CREATE TABLE events (
    ts DateTime,
    user_id UInt64,
    event String
  ) ENGINE = MergeTree
  ORDER BY (ts, user_id)
  PRIMARY KEY ts
`);

// Get ORDER BY columns
const orderBy = result.findFirst(SyntaxKind.OrderByDefinition);
const orderByCols = orderBy.findAll(SyntaxKind.ColumnReference).map(n => n.text().trim());
// → ["ts", "user_id"]

// Get PRIMARY KEY columns
const primaryKey = result.findFirst(SyntaxKind.PrimaryKeyDefinition);
const pkCols = primaryKey.findAll(SyntaxKind.ColumnReference).map(n => n.text().trim());
// → ["ts"]

// Get the engine name
const engine = result.findFirst(SyntaxKind.EngineClause);
const engineName = engine.tokenChildren().find(t => t.kind === SyntaxKind.BareWord && t.text() !== 'ENGINE')?.text();
// → "MergeTree"

// Get column definitions
const colDefs = result.findAll(SyntaxKind.ColumnDefinition).map(col => {
    const name = col.tokenChildren().find(t => t.kind === SyntaxKind.BareWord)?.text();
    const type = col.findFirst(SyntaxKind.DataType)?.text().trim();
    return { name, type };
});
// → [{ name: "ts", type: "DateTime" }, { name: "user_id", type: "UInt64" }, { name: "event", type: "String" }]
```

### Node.js usage

The library targets `--target web` (browser). In Node.js, use `initSync` with the WASM bytes directly:

```typescript
import { readFileSync } from 'fs';
import { initSync } from '@clickhouse/analyzer/pkg/clickhouse_analyzer.js';
import { parse } from '@clickhouse/analyzer';

const wasm = readFileSync('node_modules/@clickhouse/analyzer/pkg/clickhouse_analyzer_bg.wasm');
initSync({ module: new WebAssembly.Module(wasm) });

const result = parse('SELECT 1');
```
