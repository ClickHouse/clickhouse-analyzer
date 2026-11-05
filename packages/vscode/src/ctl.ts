import { execFile } from "child_process";
import { promisify } from "util";
import * as os from "os";
import * as path from "path";
import * as fs from "fs";
import * as vscode from "vscode";

const execFileAsync = promisify(execFile);

let outputChannel: vscode.OutputChannel | undefined;

function log(msg: string): void {
  if (!outputChannel) {
    outputChannel = vscode.window.createOutputChannel("ClickHouse CTL");
  }
  outputChannel.appendLine(`[${new Date().toISOString()}] ${msg}`);
}

export interface ServerInfo {
  name: string;
  status: "running" | "stopped";
  httpPort?: string;
  tcpPort?: string;
  version?: string;
  pid?: string;
}

export interface VersionInfo {
  version: string;
  active: boolean;
}

/**
 * Resolves the path to the clickhousectl binary.
 * Checks common install locations before falling back to PATH.
 */
function getCtlPath(): string {
  const home = os.homedir();
  const candidates = [
    path.join(home, ".local", "bin", "clickhousectl"),
    path.join(home, ".local", "bin", "chctl"),
  ];
  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return p;
    }
  }
  return "clickhousectl";
}

async function runCtl(args: string[], cwd?: string): Promise<string> {
  const bin = getCtlPath();
  log(`exec: ${bin} ${args.join(" ")}${cwd ? ` (cwd: ${cwd})` : ""}`);
  try {
    const { stdout, stderr } = await execFileAsync(bin, args, {
      timeout: 60_000,
      cwd,
      env: { ...process.env, NO_COLOR: "1" },
    });
    if (stderr) log(`stderr: ${stderr.trim()}`);
    log(`stdout: ${stdout.trim()}`);
    return stdout.trim();
  } catch (e: any) {
    log(`error: ${e.message}`);
    if (e.stderr) log(`stderr: ${e.stderr}`);
    throw e;
  }
}

async function runCtlJson(args: string[], cwd?: string): Promise<any> {
  const raw = await runCtl([...args, "--json"], cwd);
  try {
    return JSON.parse(raw);
  } catch {
    log(`JSON parse failed for: ${raw}`);
    throw new Error(`Failed to parse JSON from chctl: ${raw.substring(0, 200)}`);
  }
}

/** Check whether clickhousectl is available. */
export async function isCtlInstalled(): Promise<boolean> {
  try {
    await runCtl(["--version"]);
    return true;
  } catch {
    return false;
  }
}

/** Check whether the project directory has been initialized with `chctl local init`. */
export function isInitialized(workspaceRoot: string): boolean {
  return fs.existsSync(path.join(workspaceRoot, ".clickhouse"));
}

/** Initialize clickhousectl in a project directory. */
export async function init(workspaceRoot: string): Promise<string> {
  return runCtl(["local", "init"], workspaceRoot);
}

// ── Version commands (global, no cwd needed) ───────────────────────

/** List installed ClickHouse versions. */
export async function listVersions(): Promise<VersionInfo[]> {
  try {
    const data = await runCtlJson(["local", "list"]);
    log(`listVersions raw data: ${JSON.stringify(data)}`);
    const arr = Array.isArray(data) ? data : (data.versions ?? data.items ?? data.installed ?? []);
    return arr.map((item: any) => {
      log(`listVersions item: ${JSON.stringify(item)}`);
      return {
        version: String(item.version ?? item.name ?? item),
        active: Boolean(item.active ?? item.is_active ?? item.default ?? item.selected ?? false),
      };
    });
  } catch (e: any) {
    log(`listVersions failed: ${e.message}`);
    return [];
  }
}

/** Install a ClickHouse version. */
export async function installVersion(version: string): Promise<string> {
  return runCtl(["local", "install", version]);
}

/** Remove a ClickHouse version. */
export async function removeVersion(version: string): Promise<string> {
  return runCtl(["local", "remove", version]);
}

/** Set the active/default ClickHouse version. */
export async function useVersion(version: string): Promise<string> {
  return runCtl(["local", "use", version]);
}

/** List available remote versions. */
export async function listRemoteVersions(): Promise<string[]> {
  try {
    const data = await runCtlJson(["local", "list", "--remote"]);
    const arr = Array.isArray(data) ? data : (data.versions ?? data.items ?? []);
    return arr.map((item: any) => String(item.version ?? item.name ?? item));
  } catch (e: any) {
    log(`listRemoteVersions failed: ${e.message}`);
    return [];
  }
}

// ── Server commands (project-scoped, require cwd) ──────────────────

/** List local servers in the given project directory. */
export async function listServers(cwd: string): Promise<ServerInfo[]> {
  try {
    const data = await runCtlJson(["local", "server", "list"], cwd);
    log(`listServers raw data: ${JSON.stringify(data)}`);
    const arr = Array.isArray(data) ? data : (data.servers ?? data.items ?? []);
    return arr.map((item: any) => {
      log(`listServers item keys: ${Object.keys(item).join(", ")} = ${JSON.stringify(item)}`);
      return {
        name: String(item.name),
        status: (item.running === true || String(item.status ?? "").toLowerCase().includes("running"))
          ? "running" as const
          : "stopped" as const,
        httpPort: item.http_port != null ? String(item.http_port) : undefined,
        tcpPort: item.tcp_port != null ? String(item.tcp_port) : undefined,
        version: item.version != null ? String(item.version) : undefined,
        pid: item.pid != null ? String(item.pid) : undefined,
      };
    });
  } catch (e: any) {
    log(`listServers failed: ${e.message}`);
    return [];
  }
}

/** Create and start a new server. */
export async function createServer(
  cwd: string,
  name: string,
  version?: string,
): Promise<string> {
  const args = ["local", "server", "start", "--name", name];
  if (version) {
    args.push("--version", version);
  }
  return runCtl(args, cwd);
}

/** Start an existing server. */
export async function startServer(cwd: string, name: string): Promise<string> {
  return runCtl(["local", "server", "start", "--name", name], cwd);
}

/** Stop a server. */
export async function stopServer(cwd: string, name: string): Promise<string> {
  return runCtl(["local", "server", "stop", name], cwd);
}

/** Remove a server and its data. */
export async function deleteServer(cwd: string, name: string): Promise<string> {
  return runCtl(["local", "server", "remove", name], cwd);
}
