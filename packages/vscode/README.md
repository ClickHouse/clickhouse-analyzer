# ClickHouse Analyzer — VS Code Extension

> **This extension is experimental.** It is under active development and may have incomplete coverage or breaking changes between releases.

A VS Code extension for ClickHouse SQL, powered by a native LSP server built on the [clickhouse-analyzer](https://github.com/ClickHouse/clickhouse-analyzer) parser.

## Features

- **Diagnostics** — parse errors shown as squiggly underlines with enriched messages (bracket matching, clause context)
- **Formatting** — format ClickHouse SQL documents
- **Semantic highlighting** — context-aware token coloring (keywords, functions, columns, types, tables, operators, strings, numbers, comments, parameters)
- **Completions** — keyword, function, and type completions; schema-aware table and column completions when connected to a live ClickHouse instance
- **Hover** — documentation for functions, settings, and data types
- **Go to definition** — navigate to table definitions (requires live connection)

## Getting Started

1. Install the extension
2. Open any `.sql`, `.clickhouse`, or `.ch.sql` file — the analyzer starts automatically

The LSP server binary is bundled with the extension. No additional installation required.

## Live Connection (Optional)

For schema-aware completions (tables, columns), enable a connection to a running ClickHouse instance:

```json
{
  "clickhouse-analyzer.connection.enabled": true,
  "clickhouse-analyzer.connection.url": "http://localhost:8123",
  "clickhouse-analyzer.connection.database": "default",
  "clickhouse-analyzer.connection.username": "default",
  "clickhouse-analyzer.connection.password": ""
}
```

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `clickhouse-analyzer.serverPath` | `""` | Path to a custom `clickhouse-lsp` binary. If empty, uses the bundled server. |
| `clickhouse-analyzer.connection.enabled` | `false` | Enable live ClickHouse connection for schema-aware completions. |
| `clickhouse-analyzer.connection.url` | `http://localhost:8123` | ClickHouse HTTP endpoint URL. |
| `clickhouse-analyzer.connection.database` | `default` | Default database name. |
| `clickhouse-analyzer.connection.username` | `default` | Username for authentication. |
| `clickhouse-analyzer.connection.password` | `""` | Password for authentication. |
