import * as path from "path";
import * as fs from "fs";
import { workspace, window, ExtensionContext, StatusBarAlignment, StatusBarItem } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;
let statusBarItem: StatusBarItem | undefined;

function getServerPath(context: ExtensionContext): string {
  const config = workspace.getConfiguration("clickhouse-analyzer");
  const configPath = config.get<string>("serverPath");
  if (configPath) {
    return configPath;
  }

  // Look for the bundled server binary
  const ext = process.platform === "win32" ? ".exe" : "";
  const bundledPath = path.join(context.extensionPath, "server", `clickhouse-lsp${ext}`);
  if (fs.existsSync(bundledPath)) {
    return bundledPath;
  }

  // Fall back to PATH
  return "clickhouse-lsp";
}

export function activate(context: ExtensionContext) {
  const serverPath = getServerPath(context);

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

  // Status bar item showing connection state
  statusBarItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
  statusBarItem.text = "$(database) CH: Offline";
  statusBarItem.tooltip = "ClickHouse Analyzer - using compiled-in metadata";
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  // Update status bar when configuration changes
  workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration("clickhouse-analyzer.connection")) {
      updateStatusBar();
    }
  });

  // Register the log listener before starting so early messages
  // (e.g. "Connected to ClickHouse") are not missed.
  client.onNotification("window/logMessage", (params: { type: number; message: string }) => {
    if (params.message.includes("Connected to ClickHouse")) {
      const version = params.message.match(/ClickHouse (\S+)/)?.[1] || "";
      if (statusBarItem) {
        statusBarItem.text = `$(database) CH: ${version}`;
        statusBarItem.tooltip = `ClickHouse Analyzer - connected (${params.message})`;
      }
    } else if (params.message.includes("Failed to connect")) {
      if (statusBarItem) {
        statusBarItem.text = "$(database) CH: Connection Failed";
        statusBarItem.tooltip = `ClickHouse Analyzer - ${params.message}`;
      }
    } else if (params.message.includes("connection disabled")) {
      if (statusBarItem) {
        statusBarItem.text = "$(database) CH: Offline";
        statusBarItem.tooltip = "ClickHouse Analyzer - using compiled-in metadata";
      }
    }
  });

  client.start();
}

function updateStatusBar() {
  if (!statusBarItem) return;
  const config = workspace.getConfiguration("clickhouse-analyzer");
  const enabled = config.get<boolean>("connection.enabled", false);
  if (!enabled) {
    statusBarItem.text = "$(database) CH: Offline";
    statusBarItem.tooltip = "ClickHouse Analyzer - using compiled-in metadata";
  }
}

export function deactivate(): Thenable<void> | undefined {
  statusBarItem?.dispose();
  return client?.stop();
}
