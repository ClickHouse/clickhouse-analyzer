//! Codegen tool: queries ClickHouse and writes metadata JSON files
//! to `generated/` for compilation into the analyzer binary.
//!
//! Supports two modes:
//!
//! 1. clickhouse-local (default, no server needed):
//!    CLICKHOUSE_BIN=clickhouse cargo run --bin codegen --features codegen
//!
//! 2. HTTP (requires a running server):
//!    CLICKHOUSE_URL=http://localhost:8123 cargo run --bin codegen --features codegen
//!
//! In CI, download the ClickHouse binary and use mode 1.

use clickhouse_analyzer::metadata::types::*;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Query backend — either clickhouse-local or HTTP.
enum Backend {
    Local { binary: String },
    Http(clickhouse_analyzer::connection::client::ClickHouseClient),
}

impl Backend {
    async fn query_json<T: DeserializeOwned>(&self, sql: &str) -> Result<Vec<T>, Box<dyn std::error::Error>> {
        match self {
            Backend::Local { binary } => {
                let output = Command::new(binary)
                    .args(["local", "--query", sql, "--format", "JSONEachRow"])
                    .output()
                    .map_err(|e| format!("failed to execute '{binary}': {e}"))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("clickhouse-local query failed (exit {}):\n  query: {}\n  stderr: {stderr}",
                        output.status, sql).into());
                }
                let text = String::from_utf8(output.stdout)?;
                text.lines()
                    .filter(|line| !line.is_empty())
                    .map(|line| serde_json::from_str(line).map_err(Into::into))
                    .collect()
            }
            Backend::Http(client) => {
                Ok(client.query_json(sql).await?)
            }
        }
    }

    async fn query_text(&self, sql: &str) -> Result<String, Box<dyn std::error::Error>> {
        match self {
            Backend::Local { binary } => {
                let output = Command::new(binary)
                    .args(["local", "--query", sql])
                    .output()
                    .map_err(|e| format!("failed to execute '{binary}': {e}"))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("clickhouse-local query failed (exit {}):\n  query: {}\n  stderr: {stderr}",
                        output.status, sql).into());
                }
                Ok(String::from_utf8(output.stdout)?)
            }
            Backend::Http(client) => {
                Ok(client.query_text(sql).await?)
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = if let Ok(url) = std::env::var("CLICKHOUSE_URL") {
        // HTTP mode
        let config = clickhouse_analyzer::connection::client::ConnectionConfig {
            url: url.clone(),
            database: std::env::var("CLICKHOUSE_DATABASE").unwrap_or_else(|_| "default".into()),
            username: std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".into()),
            password: std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
        };
        eprintln!("Connecting to {url}...");
        let client = clickhouse_analyzer::connection::client::ClickHouseClient::new(config)?;
        client.ping().await?;
        Backend::Http(client)
    } else {
        // clickhouse-local mode
        let binary = std::env::var("CLICKHOUSE_BIN").unwrap_or_else(|_| "clickhouse".into());
        // Verify binary exists
        let check = Command::new(&binary).args(["local", "--query", "SELECT 1"]).output();
        match check {
            Ok(o) if o.status.success() => {}
            _ => {
                eprintln!("Error: Cannot find or run '{binary}'.");
                eprintln!("Set CLICKHOUSE_BIN to the path of the clickhouse binary,");
                eprintln!("or set CLICKHOUSE_URL for HTTP mode.");
                std::process::exit(1);
            }
        }
        eprintln!("Using clickhouse-local: {binary}");
        Backend::Local { binary }
    };

    let version = backend
        .query_text("SELECT version()")
        .await?
        .trim()
        .to_string();
    eprintln!("ClickHouse {version}");

    let out_dir = Path::new("generated");
    fs::create_dir_all(out_dir)?;

    write_query::<FunctionInfo>(
        &backend,
        "SELECT name, is_aggregate, case_insensitive, alias_to, \
         syntax, arguments, returned_value, description, categories \
         FROM system.functions ORDER BY name",
        &out_dir.join("functions.json"),
    ).await?;

    write_query::<SettingInfo>(
        &backend,
        "SELECT name, description, type, default, tier \
         FROM system.settings WHERE is_obsolete = 0 ORDER BY name",
        &out_dir.join("settings.json"),
    ).await?;

    write_query::<SettingInfo>(
        &backend,
        "SELECT name, description, type, default, tier \
         FROM system.merge_tree_settings WHERE is_obsolete = 0 ORDER BY name",
        &out_dir.join("merge_tree_settings.json"),
    ).await?;

    write_query::<DataTypeInfo>(
        &backend,
        "SELECT name, case_insensitive, alias_to \
         FROM system.data_type_families ORDER BY name",
        &out_dir.join("data_types.json"),
    ).await?;

    write_query::<TableEngineInfo>(
        &backend,
        "SELECT name, supports_settings, supports_sort_order, \
         supports_ttl, supports_replication \
         FROM system.table_engines ORDER BY name",
        &out_dir.join("table_engines.json"),
    ).await?;

    write_query::<FormatInfo>(
        &backend,
        "SELECT name, is_input, is_output FROM system.formats ORDER BY name",
        &out_dir.join("formats.json"),
    ).await?;

    write_query::<CodecInfo>(
        &backend,
        "SELECT name FROM system.codecs ORDER BY name",
        &out_dir.join("codecs.json"),
    ).await?;

    // Keywords (system.keywords available since CH 24.4)
    match backend.query_text("SELECT keyword FROM system.keywords ORDER BY keyword").await {
        Ok(text) => {
            let keywords: Vec<String> = text.lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.trim().to_string())
                .collect();
            let json = serde_json::to_string_pretty(&keywords)?;
            fs::write(out_dir.join("keywords.json"), &json)?;
            eprintln!("  keywords: {} entries", keywords.len());
        }
        Err(e) => {
            eprintln!("  keywords: skipped ({e}), keeping existing file");
        }
    }

    fs::write(out_dir.join("version.txt"), &version)?;
    eprintln!("Done. Generated files written to {}", out_dir.display());

    Ok(())
}

async fn write_query<T: DeserializeOwned + serde::Serialize>(
    backend: &Backend,
    sql: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let rows: Vec<T> = backend.query_json(sql).await?;
    let json = serde_json::to_string_pretty(&rows)?;
    fs::write(path, &json)?;
    eprintln!(
        "  {}: {} entries",
        path.file_name().unwrap().to_string_lossy(),
        rows.len()
    );
    Ok(())
}
