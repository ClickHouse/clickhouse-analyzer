import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import { workspace, window, commands, ExtensionContext, StatusBarAlignment, StatusBarItem, Uri } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient/node";
import * as ctl from "./ctl";
import { ServerTreeProvider, VersionTreeProvider } from "./sidebar";

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

/** Returns the first workspace folder path, or undefined if none open. */
function getWorkspaceRoot(): string | undefined {
  return workspace.workspaceFolders?.[0]?.uri.fsPath;
}

export function activate(context: ExtensionContext) {
  // ── LSP client setup ─────────────────────────────────────────────

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

  // ── Status bar ───────────────────────────────────────────────────

  statusBarItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
  statusBarItem.text = "$(database) CH: Offline";
  statusBarItem.tooltip = "ClickHouse Analyzer - using compiled-in metadata";
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration("clickhouse-analyzer.connection")) {
      updateStatusBar();
    }
  });

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

  // ── Workspace & project state ────────────────────────────────────

  const serverTree = new ServerTreeProvider();
  const versionTree = new VersionTreeProvider();

  async function refreshCtlState() {
    const installed = await ctl.isCtlInstalled();
    commands.executeCommand("setContext", "clickhouse-analyzer.ctlInstalled", installed);
  }

  function refreshProjectState() {
    const root = getWorkspaceRoot();
    const initialized = root ? ctl.isInitialized(root) : false;
    commands.executeCommand("setContext", "clickhouse-analyzer.projectInitialized", initialized);
    serverTree.workspaceRoot = initialized ? root : undefined;
  }

  // Set initial state
  refreshCtlState();
  refreshProjectState();

  // Re-check when workspace folders change (open/close folder)
  context.subscriptions.push(
    workspace.onDidChangeWorkspaceFolders(() => refreshProjectState()),
  );

  // Watch for .clickhouse directory creation (e.g. init from terminal)
  const root = getWorkspaceRoot();
  if (root) {
    const watcher = workspace.createFileSystemWatcher(
      new (require("vscode").RelativePattern)(root, ".clickhouse/**"),
    );
    watcher.onDidCreate(() => refreshProjectState());
    watcher.onDidDelete(() => refreshProjectState());
    context.subscriptions.push(watcher);
  }

  // ── Sidebar tree views ───────────────────────────────────────────

  const serverView = window.createTreeView("clickhouse-servers", {
    treeDataProvider: serverTree,
    showCollapseAll: false,
  });
  const versionView = window.createTreeView("clickhouse-versions", {
    treeDataProvider: versionTree,
    showCollapseAll: false,
  });

  context.subscriptions.push(serverView, versionView);

  // ── Commands ─────────────────────────────────────────────────────

  context.subscriptions.push(
    commands.registerCommand("clickhouse-analyzer.refreshServers", async () => {
      await refreshCtlState();
      refreshProjectState();
      await serverTree.reload();
    }),
    commands.registerCommand("clickhouse-analyzer.refreshVersions", async () => {
      await refreshCtlState();
      await versionTree.reload();
    }),

    commands.registerCommand("clickhouse-analyzer.installCtl", async () => {
      const installed = await ctl.isCtlInstalled();
      if (installed) {
        window.showInformationMessage("clickhousectl is already installed.");
        return;
      }

      const choice = await window.showInformationMessage(
        "Install clickhousectl via the official install script?",
        "Install",
        "Cancel",
      );
      if (choice !== "Install") return;

      let terminal = window.terminals.find((t) => t.name === "clickhousectl");
      if (!terminal) {
        terminal = window.createTerminal("clickhousectl");
      }
      terminal.show();
      terminal.sendText("curl -fsSL https://clickhouse.com/cli | sh");

      window.showInformationMessage(
        "Installing clickhousectl… Refresh the sidebar once complete.",
      );
    }),

    commands.registerCommand("clickhouse-analyzer.initProject", async () => {
      if (!(await ensureCtl())) return;

      const wsRoot = getWorkspaceRoot();
      if (!wsRoot) {
        window.showErrorMessage("Open a folder first to initialize ClickHouse.");
        return;
      }

      if (ctl.isInitialized(wsRoot)) {
        window.showInformationMessage("ClickHouse is already initialized in this project.");
        refreshProjectState();
        return;
      }

      await window.withProgress(
        { location: { viewId: "clickhouse-servers" }, title: "Initializing ClickHouse…" },
        async () => {
          try {
            await ctl.init(wsRoot);
            refreshProjectState();
            await serverTree.reload();
            window.showInformationMessage("ClickHouse initialized in this project.");
          } catch (e: any) {
            window.showErrorMessage(`Failed to initialize: ${e.message}`);
          }
        },
      );
    }),

    commands.registerCommand("clickhouse-analyzer.installVersion", async () => {
      if (!(await ensureCtl())) return;

      const version = await window.showInputBox({
        prompt: "ClickHouse version to install (e.g. stable, lts, 25.3)",
        placeHolder: "stable",
        value: "stable",
      });
      if (!version) return;

      await window.withProgress(
        { location: { viewId: "clickhouse-versions" }, title: `Installing ${version}…` },
        async () => {
          try {
            await ctl.installVersion(version);
            window.showInformationMessage(`ClickHouse ${version} installed.`);
            await versionTree.reload();
          } catch (e: any) {
            window.showErrorMessage(`Failed to install: ${e.message}`);
          }
        },
      );
    }),

    commands.registerCommand("clickhouse-analyzer.useVersion", async (item) => {
      if (!(await ensureCtl())) return;
      const version: string = item?.info?.version;
      if (!version) return;

      try {
        await ctl.useVersion(version);
        window.showInformationMessage(`Default ClickHouse version set to ${version}.`);
        await versionTree.reload();
      } catch (e: any) {
        window.showErrorMessage(`Failed to set version: ${e.message}`);
      }
    }),

    commands.registerCommand("clickhouse-analyzer.removeVersion", async (item) => {
      if (!(await ensureCtl())) return;
      const version: string = item?.info?.version;
      if (!version) return;

      const confirm = await window.showWarningMessage(
        `Remove ClickHouse ${version}?`,
        { modal: true },
        "Remove",
      );
      if (confirm !== "Remove") return;

      try {
        await ctl.removeVersion(version);
        window.showInformationMessage(`ClickHouse ${version} removed.`);
        await versionTree.reload();
      } catch (e: any) {
        window.showErrorMessage(`Failed to remove: ${e.message}`);
      }
    }),

    commands.registerCommand("clickhouse-analyzer.createServer", async () => {
      if (!(await ensureCtl())) return;
      const wsRoot = requireWorkspace();
      if (!wsRoot) return;

      const name = await window.showInputBox({
        prompt: "Server name",
        placeHolder: "default",
        value: "default",
      });
      if (!name) return;

      const versions = await ctl.listVersions();
      let version: string | undefined;
      if (versions.length > 0) {
        const picked = await window.showQuickPick(
          versions.map((v) => ({
            label: v.version,
            description: v.active ? "(active)" : "",
          })),
          { placeHolder: "Select ClickHouse version (or Escape for default)" },
        );
        version = picked?.label;
      }

      await window.withProgress(
        { location: { viewId: "clickhouse-servers" }, title: `Creating server ${name}…` },
        async () => {
          try {
            await ctl.createServer(wsRoot, name, version);
            window.showInformationMessage(`Server "${name}" created and started.`);
            await serverTree.reload();
          } catch (e: any) {
            window.showErrorMessage(`Failed to create server: ${e.message}`);
          }
        },
      );
    }),

    commands.registerCommand("clickhouse-analyzer.startServer", async (item) => {
      if (!(await ensureCtl())) return;
      const wsRoot = requireWorkspace();
      if (!wsRoot) return;
      const name: string = item?.server?.name;
      if (!name) return;

      await window.withProgress(
        { location: { viewId: "clickhouse-servers" }, title: `Starting ${name}…` },
        async () => {
          try {
            await ctl.startServer(wsRoot, name);
            await serverTree.reload();
          } catch (e: any) {
            window.showErrorMessage(`Failed to start server: ${e.message}`);
          }
        },
      );
    }),

    commands.registerCommand("clickhouse-analyzer.stopServer", async (item) => {
      if (!(await ensureCtl())) return;
      const wsRoot = requireWorkspace();
      if (!wsRoot) return;
      const name: string = item?.server?.name;
      if (!name) return;

      try {
        await ctl.stopServer(wsRoot, name);
        if (serverTree.connectedServer === name) {
          serverTree.connectedServer = undefined;
          await disconnectLsp();
        }
        await serverTree.reload();
      } catch (e: any) {
        window.showErrorMessage(`Failed to stop server: ${e.message}`);
      }
    }),

    commands.registerCommand("clickhouse-analyzer.deleteServer", async (item) => {
      if (!(await ensureCtl())) return;
      const wsRoot = requireWorkspace();
      if (!wsRoot) return;
      const name: string = item?.server?.name;
      if (!name) return;

      const confirm = await window.showWarningMessage(
        `Delete server "${name}"? This removes all its data.`,
        { modal: true },
        "Delete",
      );
      if (confirm !== "Delete") return;

      try {
        // Stop first if running
        if (item.server.status === "running") {
          await ctl.stopServer(wsRoot, name);
        }
        await ctl.deleteServer(wsRoot, name);
        if (serverTree.connectedServer === name) {
          serverTree.connectedServer = undefined;
          await disconnectLsp();
        }
        window.showInformationMessage(`Server "${name}" deleted.`);
        await serverTree.reload();
      } catch (e: any) {
        window.showErrorMessage(`Failed to delete server: ${e.message}`);
      }
    }),

    commands.registerCommand("clickhouse-analyzer.connectLsp", async (item) => {
      const server: ctl.ServerInfo = item?.server;
      if (!server || server.status !== "running") {
        window.showErrorMessage("Server must be running to connect.");
        return;
      }

      const port = server.httpPort || "8123";
      const url = `http://localhost:${port}`;

      const config = workspace.getConfiguration("clickhouse-analyzer");
      await config.update("connection.enabled", true, true);
      await config.update("connection.url", url, true);
      await config.update("connection.database", "default", true);
      await config.update("connection.username", "default", true);
      await config.update("connection.password", "", true);

      serverTree.connectedServer = server.name;
      window.showInformationMessage(
        `LSP connected to "${server.name}" at ${url}`,
      );
    }),

    commands.registerCommand("clickhouse-analyzer.addToEnv", async (item) => {
      const server: ctl.ServerInfo = item?.server;
      if (!server || server.status !== "running") {
        window.showErrorMessage("Server must be running to export connection details.");
        return;
      }

      const wsRoot = getWorkspaceRoot();
      if (!wsRoot) {
        window.showErrorMessage("Open a folder first.");
        return;
      }

      const envPath = path.join(wsRoot, ".env");
      const vars: Record<string, string> = {
        CLICKHOUSE_HOST: "localhost",
        CLICKHOUSE_PORT: server.httpPort || "8123",
        CLICKHOUSE_TCP_PORT: server.tcpPort || "9000",
        CLICKHOUSE_URL: `http://localhost:${server.httpPort || "8123"}`,
        CLICKHOUSE_USER: "default",
        CLICKHOUSE_PASSWORD: "",
        CLICKHOUSE_DB: "default",
      };

      // Read existing .env if it exists
      let existing = "";
      if (fs.existsSync(envPath)) {
        existing = fs.readFileSync(envPath, "utf-8");
      }

      // Update or append each var
      let updated = existing;
      for (const [key, value] of Object.entries(vars)) {
        const regex = new RegExp(`^${key}=.*$`, "m");
        const line = `${key}=${value}`;
        if (regex.test(updated)) {
          updated = updated.replace(regex, line);
        } else {
          if (updated.length > 0 && !updated.endsWith("\n")) {
            updated += os.EOL;
          }
          updated += line + os.EOL;
        }
      }

      fs.writeFileSync(envPath, updated);

      const doc = await workspace.openTextDocument(Uri.file(envPath));
      await window.showTextDocument(doc);
      window.showInformationMessage(
        `ClickHouse connection details written to .env`,
      );
    }),
  );
}

// ── Helpers ────────────────────────────────────────────────────────

/** Returns workspace root or shows an error and returns undefined. */
function requireWorkspace(): string | undefined {
  const root = getWorkspaceRoot();
  if (!root) {
    window.showErrorMessage("Open a folder first to manage local ClickHouse servers.");
    return undefined;
  }
  if (!ctl.isInitialized(root)) {
    window.showErrorMessage(
      "ClickHouse is not initialized in this project. Run \"Initialize ClickHouse in Project\" first.",
    );
    return undefined;
  }
  return root;
}

async function ensureCtl(): Promise<boolean> {
  const installed = await ctl.isCtlInstalled();
  if (!installed) {
    const choice = await window.showErrorMessage(
      "clickhousectl is not installed. Install it to manage local servers.",
      "Install",
      "Cancel",
    );
    if (choice === "Install") {
      commands.executeCommand("clickhouse-analyzer.installCtl");
    }
    return false;
  }
  return true;
}

async function disconnectLsp(): Promise<void> {
  const config = workspace.getConfiguration("clickhouse-analyzer");
  await config.update("connection.enabled", false, true);
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
