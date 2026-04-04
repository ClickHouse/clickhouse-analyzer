# ClickHouse Analyzer — LSP Intelligence Layer

This document describes the work done on the `feature/lsp-intelligence` branch, which adds intelligent LSP features on top of the existing clickhouse-analyzer parser.

## Overview

The clickhouse-analyzer project (by Spencer Torres at ClickHouse) provides a resilient hand-written recursive descent parser for ClickHouse SQL (~95% coverage), a formatter, basic diagnostics, semantic token highlighting, and a minimal VS Code extension. This branch adds the "intelligence layer" — completions, hover, go-to-definition, signature help, server-side validation, and a three-tier metadata system that works offline and upgrades when connected to a ClickHouse instance.

**Branch:** `feature/lsp-intelligence` (31 commits)
**Fork:** `sdairs/clickhouse-analyzer` (intended to merge upstream to `ClickHouse/clickhouse-analyzer`)

---

## Architecture: Three-Tier Metadata

The core design principle: metadata is **not hand-maintained** and **not runtime-only**. It uses a three-tier approach:

### Tier 1: Compiled-in defaults (offline, zero config)
- At build time, a codegen tool queries a ClickHouse instance and serializes all global metadata into JSON files under `generated/`
- These are embedded into the binary via `include_str!()` and deserialized on startup
- Current snapshot: ClickHouse 26.4.1.485 (1,790 functions, 1,433 settings, 300 MergeTree settings, 140 data types, 74 engines, 108 formats, 14 codecs, 620 keywords)
- Users get completions/hover/signature help immediately with zero configuration

### Tier 2: Live server overlay (connected)
- When connected to a ClickHouse instance, live-queries system tables to replace compiled-in defaults
- Handles version mismatch (user running different CH version than analyzer was built against)
- Adds UDFs which are instance-specific

### Tier 3: Schema-aware (connected + database context)
- Databases, tables, columns — always runtime-only (instance-specific)
- Lazy-loaded per database/table on demand
- Enables column-level completions, table hover with column list, semantic validation

---

## New Modules

### `src/metadata/` — Metadata types, generated defaults, and cache

| File | Purpose |
|------|---------|
| `types.rs` | Data types for all ClickHouse metadata (FunctionInfo, SettingInfo, DataTypeInfo, TableEngineInfo, FormatInfo, CodecInfo, DatabaseInfo, TableInfo, ColumnInfo). Shared by codegen and runtime. All use `serde` with custom `bool_from_int` deserializer for ClickHouse's 0/1 booleans. |
| `generated.rs` | `include_str!()` references to the 8 JSON files in `generated/`. Constants for compiled-in metadata. |
| `cache.rs` | `MetadataCache` struct implementing the three-tier strategy. `from_compiled_defaults()` for Tier 1, `connect()` for Tier 2, `ensure_tables()`/`ensure_columns()` for Tier 3. Shared across handlers via `Arc<RwLock<MetadataCache>>`. |

### `src/connection/` — ClickHouse HTTP client

| File | Purpose |
|------|---------|
| `client.rs` | `ClickHouseClient` using `reqwest` for HTTP. Supports `query_json<T>()` (JSONEachRow deserialization) and `query_text()`. Basic auth, configurable timeouts. `ConnectionConfig` for URL/database/username/password. |

### `src/analysis/` — CST intelligence (no external dependencies)

| File | Purpose |
|------|---------|
| `cursor_context.rs` | Determines what the user is typing at a given byte offset in the CST. Returns a `CursorContext` enum: `SelectExpression`, `TableReference`, `ColumnOfTable`, `Expression`, `SettingName`, `EngineName`, `FormatName`, `DataType`, `CodecName`, `FunctionArgument`, `StatementStart`, etc. Handles error recovery (Error nodes, trailing clauses with empty content). 19 unit tests. |
| `scope.rs` | Extracts CTEs, table aliases, column aliases, and table references from a query's CST. `build_scope()` walks WithClause, FromClause, JoinClause, SelectClause. `find_enclosing_statement()` for go-to-definition context. 4 unit tests. |

### `src/lsp/` — New LSP handlers

| File | Purpose |
|------|---------|
| `completion.rs` | Context-aware completions. Maps each `CursorContext` to appropriate completion items: functions after SELECT, tables after FROM, columns after `table.`, settings after SETTINGS, engines after ENGINE=, formats after FORMAT, data types in column definitions, codecs in CODEC(). Filters internal `__` functions, case-insensitive sorting, pre-fetches schema with write lock then reads. |
| `hover.rs` | Hover information keyed on parent SyntaxKind: function signatures+docs from metadata, setting descriptions with type/default, data type alias info, engine capabilities, table column list (when connected). |
| `goto_definition.rs` | Pure CST operation — resolves CTE references, table aliases, and column aliases to their definition locations within the same query. |
| `signature_help.rs` | Function parameter help. Parses the `syntax` field from FunctionInfo to extract parameter names. Tracks active parameter via comma counting in the CST. |
| `document_symbols.rs` | Outline sidebar — each statement as a top-level symbol with CTEs, table references (with aliases), and column aliases as children. Labels show first few tokens. |

### `src/bin/codegen.rs` — Build-time metadata generator

Standalone binary that queries ClickHouse system tables and writes JSON to `generated/`. Supports two modes:
1. **clickhouse-local** (default): `CLICKHOUSE_BIN=path/to/clickhouse cargo run --bin codegen --features codegen` — no server needed
2. **HTTP**: `CLICKHOUSE_URL=http://localhost:8123 cargo run --bin codegen --features codegen` — against a running server

### `tests/corpus.rs` — Parser coverage test

Parses all ~7,668 `.sql` files from `ClickHouse/tests/queries` and reports coverage. Current baseline: 82.1% of files parse clean (6,282/7,656). Informational — does not fail. Machine-readable output for CI.

---

## Modifications to Existing Files

| File | Change | Impact |
|------|--------|--------|
| `Cargo.toml` | Added `reqwest` dependency, `codegen` feature, codegen binary target. Extended `lsp` feature to include `reqwest` and `serde`. | Low — additive |
| `src/lib.rs` | Added `pub mod metadata` (behind `serde`), `pub mod connection` (behind `lsp`/`codegen`), `pub mod analysis` (behind `lsp`) | Low — 3 lines |
| `src/parser/syntax_tree.rs` | Added `#[derive(Clone)]` to `SyntaxTree` and `SyntaxChild` | Low — enables cloning CST for async handlers |
| `src/parser/diagnostic.rs` | Added `#[derive(Clone)]` to `Parse` | Low — same reason |
| `src/lsp/line_index.rs` | Added `#[derive(Clone)]` to `LineIndex` | Low — same reason |
| `src/lsp/mod.rs` | Major changes: added `metadata` field to Backend (initialized from compiled defaults), registered 5 new capabilities (completion, hover, definition, signatureHelp, documentSymbol), implemented 5 new LanguageServer trait methods, added connection lifecycle (`try_connect`, `did_change_configuration`, config request on startup), parse caching in `DocumentState`, server-side validation via EXPLAIN PLAN with multi-statement splitting, `refresh_all_diagnostics` after connecting | Medium — core integration point |
| `packages/vscode/package.json` | Added 5 connection configuration properties, renamed package for vsix compat, added `.vscodeignore` | Low |
| `packages/vscode/src/extension.ts` | Added `synchronize.configurationSection`, connection status bar item (listens for LSP log messages), untitled file support | Low |

---

## CI Workflows

### `.github/workflows/ci.yml`
- **Trigger:** Push to main/feature/**, PRs to main
- **Jobs:** `cargo check` (default, lsp, codegen features), `cargo test --features lsp`, `cargo clippy --features lsp`

### `.github/workflows/corpus-coverage.yml`
- **Trigger:** Push to main/feature/**
- **What:** Sparse-checkouts `ClickHouse/tests/queries`, parses all .sql files, posts coverage table to GitHub step summary
- **Purpose:** Track parser coverage over time

### `.github/workflows/codegen-update.yml`
- **Trigger:** Weekly Monday 06:00 UTC + manual dispatch
- **What:** Installs clickhousectl via `https://clickhouse.com/cli`, installs latest stable ClickHouse, runs codegen via clickhouse-local, opens PR if metadata changed
- **Purpose:** Keep compiled-in metadata in sync with ClickHouse releases automatically

---

## LSP Capabilities Summary

| Capability | LSP Method | Trigger | Tier | Notes |
|-----------|------------|---------|------|-------|
| Completions | `textDocument/completion` | `.` and ` ` | 1/2/3 | Context-aware: functions, keywords, tables, columns, settings, engines, formats, types, codecs |
| Hover | `textDocument/hover` | Mouse hover | 1/2/3 | Function docs, setting info, type aliases, engine features, table columns |
| Go-to-definition | `textDocument/definition` | Cmd+click | N/A | CTEs, table aliases, column aliases (pure CST, no connection) |
| Signature help | `textDocument/signatureHelp` | `(` and `,` | 1/2 | Function parameter names and active param tracking |
| Document symbols | `textDocument/documentSymbol` | Outline panel | N/A | Statements with CTEs, tables, aliases as children |
| Server validation | `textDocument/publishDiagnostics` | On change | 2/3 | EXPLAIN PLAN validates columns/tables/types, highlights offending identifier |
| Formatting | `textDocument/formatting` | Shift+Alt+F | N/A | Existing (by Spencer) |
| Semantic tokens | `textDocument/semanticTokens/full` | Automatic | N/A | Existing (by Spencer) |
| Diagnostics | `textDocument/publishDiagnostics` | On change | N/A | Existing syntax errors (by Spencer) + server validation (new) |

---

## Generated Metadata (`generated/`)

Built from ClickHouse 26.4.1.485:

| File | Entries | Size |
|------|--------|------|
| `functions.json` | 1,790 | 1.5 MB |
| `settings.json` | 1,433 | 590 KB |
| `merge_tree_settings.json` | 300 | 129 KB |
| `data_types.json` | 140 | 12 KB |
| `table_engines.json` | 74 | 12 KB |
| `formats.json` | 108 | 9.2 KB |
| `keywords.json` | 620 | 9.4 KB |
| `codecs.json` | 14 | 427 B |
| `version.txt` | — | 10 B |

---

## Testing

- **556 unit tests** (all passing) — parser, formatter, diagnostics, cursor context (19 tests), scope (4 tests), signature help params (4 tests)
- **Corpus test** — 7,668 .sql files from ClickHouse repo, 82.1% file coverage
- **Smoke test** (`test_lsp.py`) — JSON-RPC over stdio, tests all 6 LSP features end-to-end

---

## VS Code Extension

- **Package:** `clickhouse-analyzer-0.1.0.vsix` (434 KB)
- **Build:** `cd packages/vscode && npx @vscode/vsce package --allow-missing-repository`
- **Install:** `code --install-extension clickhouse-analyzer-0.1.0.vsix`
- **Settings:**
  - `clickhouse-analyzer.serverPath` — path to `clickhouse-lsp` binary
  - `clickhouse-analyzer.connection.enabled` — enable live ClickHouse connection
  - `clickhouse-analyzer.connection.url` — HTTP endpoint (default: `http://localhost:8123`)
  - `clickhouse-analyzer.connection.database` — default database
  - `clickhouse-analyzer.connection.username` / `password` — authentication
- **Status bar:** Shows "CH: Offline", "CH: {version}", or "CH: Connection Failed"
- **Compatibility:** VS Code ^1.75.0, Cursor

---

## Building

```bash
# Build the LSP server
cargo build --release --features lsp

# Build the VS Code extension
cd packages/vscode && npm install && npm run build
npx @vscode/vsce package --allow-missing-repository

# Regenerate compiled-in metadata (requires clickhouse binary)
CLICKHOUSE_BIN=/path/to/clickhouse cargo run --bin codegen --features codegen

# Run tests
cargo test --features lsp

# Run corpus coverage test
CLICKHOUSE_QUERIES_PATH=/path/to/ClickHouse/tests/queries cargo test --test corpus -- --nocapture
```

---

## Known Limitations / Future Work

- **No multiple connection support** — single connection only, users with dev/staging/prod need to switch manually
- **Column completions require connection** — without Tier 3, only functions/keywords/settings complete (still useful)
- **EXPLAIN PLAN validation limited to SELECT/WITH/INSERT** — DDL statements (CREATE, ALTER, DROP) are skipped since EXPLAIN PLAN can't handle them
- **No incremental parsing** — full re-parse on every change (fast enough for now, the parser is sub-millisecond for typical files)
- **Parser coverage at 82.1%** — this is the upstream parser's coverage, not our code. Top gaps are KQL files (not ClickHouse SQL) and some advanced syntax
