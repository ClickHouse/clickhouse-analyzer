import { workspace, ExtensionContext } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext) {
  const config = workspace.getConfiguration("clickhouse-analyzer");
  const serverPath =
    config.get<string>("serverPath") || "clickhouse-lsp";

  const run: Executable = {
    command: serverPath,
  };

  const serverOptions: ServerOptions = { run, debug: run };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "sql" },
      { scheme: "file", language: "clickhouse" },
      { scheme: "file", pattern: "**/*.ch.sql" },
      { scheme: "untitled", language: "sql" },
      { scheme: "untitled", language: "clickhouse" },
    ],
    synchronize: {
      configurationSection: "clickhouse-analyzer",
    },
  };

  client = new LanguageClient(
    "clickhouse-analyzer",
    "ClickHouse Analyzer",
    serverOptions,
    clientOptions,
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
