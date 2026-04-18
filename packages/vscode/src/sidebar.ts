import * as vscode from "vscode";
import * as ctl from "./ctl";

// ── Server tree ──────────────────────────────────────────────────────

export class ServerTreeProvider implements vscode.TreeDataProvider<ServerItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<ServerItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private servers: ctl.ServerInfo[] = [];
  private _connectedServer: string | undefined;
  private _workspaceRoot: string | undefined;

  get connectedServer(): string | undefined {
    return this._connectedServer;
  }

  set connectedServer(name: string | undefined) {
    this._connectedServer = name;
    this.refresh();
  }

  get workspaceRoot(): string | undefined {
    return this._workspaceRoot;
  }

  set workspaceRoot(root: string | undefined) {
    this._workspaceRoot = root;
    this.servers = [];
    this.refresh();
  }

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  async reload(): Promise<void> {
    if (!this._workspaceRoot) {
      this.servers = [];
    } else {
      this.servers = await ctl.listServers(this._workspaceRoot);
    }
    this.refresh();
  }

  getTreeItem(element: ServerItem): vscode.TreeItem {
    return element;
  }

  async getChildren(): Promise<ServerItem[]> {
    if (!this._workspaceRoot) return [];

    if (this.servers.length === 0) {
      this.servers = await ctl.listServers(this._workspaceRoot);
    }
    return this.servers.map((s) => new ServerItem(s, s.name === this._connectedServer));
  }
}

export class ServerItem extends vscode.TreeItem {
  constructor(
    public readonly server: ctl.ServerInfo,
    public readonly isConnected: boolean,
  ) {
    super(server.name, vscode.TreeItemCollapsibleState.None);

    const running = server.status === "running";
    const icon = isConnected
      ? new vscode.ThemeIcon("plug", new vscode.ThemeColor("charts.green"))
      : running
        ? new vscode.ThemeIcon("circle-filled", new vscode.ThemeColor("charts.green"))
        : new vscode.ThemeIcon("circle-outline");

    this.iconPath = icon;
    this.contextValue = running ? "server-running" : "server-stopped";

    const parts: string[] = [server.status];
    if (running && server.httpPort) parts.push(`HTTP :${server.httpPort}`);
    if (running && server.version) parts.push(`v${server.version}`);
    if (isConnected) parts.push("(LSP connected)");
    this.description = parts.join(" · ");

    this.tooltip = [
      `Server: ${server.name}`,
      `Status: ${server.status}`,
      server.httpPort ? `HTTP port: ${server.httpPort}` : null,
      server.tcpPort ? `TCP port: ${server.tcpPort}` : null,
      server.version ? `Version: ${server.version}` : null,
      server.pid ? `PID: ${server.pid}` : null,
      isConnected ? "\n✓ LSP is connected to this server" : null,
    ]
      .filter(Boolean)
      .join("\n");
  }
}

// ── Version tree ─────────────────────────────────────────────────────

export class VersionTreeProvider implements vscode.TreeDataProvider<VersionItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<VersionItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private versions: ctl.VersionInfo[] = [];

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  async reload(): Promise<void> {
    this.versions = await ctl.listVersions();
    this.refresh();
  }

  getTreeItem(element: VersionItem): vscode.TreeItem {
    return element;
  }

  async getChildren(): Promise<VersionItem[]> {
    if (this.versions.length === 0) {
      this.versions = await ctl.listVersions();
    }
    return this.versions.map((v) => new VersionItem(v));
  }
}

export class VersionItem extends vscode.TreeItem {
  constructor(public readonly info: ctl.VersionInfo) {
    super(info.version, vscode.TreeItemCollapsibleState.None);

    this.iconPath = info.active
      ? new vscode.ThemeIcon("star-full", new vscode.ThemeColor("charts.yellow"))
      : new vscode.ThemeIcon("package");

    this.contextValue = info.active ? "version-active" : "version";
    if (info.active) {
      this.description = "(default)";
    }
  }
}
