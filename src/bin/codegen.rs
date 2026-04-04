//! Codegen tool: queries a ClickHouse instance and writes metadata JSON files
//! to `generated/` for compilation into the analyzer binary.
//!
//! Usage:
//!   CLICKHOUSE_URL=http://localhost:8123 cargo run --bin codegen --features codegen

use clickhouse_analyzer::connection::client::{ClickHouseClient, ConnectionConfig};
use clickhouse_analyzer::metadata::types::*;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".into());
    let database =
        std::env::var("CLICKHOUSE_DATABASE").unwrap_or_else(|_| "default".into());
    let username =
        std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".into());
    let password = std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default();

    let config = ConnectionConfig {
        url,
        database,
        username,
        password,
    };

    eprintln!("Connecting to {}...", config.url);
    let client = ClickHouseClient::new(config)?;
    client.ping().await?;

    let version = client
        .query_text("SELECT version()")
        .await?
        .trim()
        .to_string();
    eprintln!("Connected to ClickHouse {version}");

    let out_dir = Path::new("generated");
    fs::create_dir_all(out_dir)?;

    // Query and write each metadata type
    write_query::<FunctionInfo>(
        &client,
        "SELECT name, is_aggregate, case_insensitive, alias_to, \
         syntax, arguments, returned_value, description, categories \
         FROM system.functions ORDER BY name",
        &out_dir.join("functions.json"),
    )
    .await?;

    write_query::<SettingInfo>(
        &client,
        "SELECT name, description, type, default, tier \
         FROM system.settings WHERE is_obsolete = 0 ORDER BY name",
        &out_dir.join("settings.json"),
    )
    .await?;

    write_query::<SettingInfo>(
        &client,
        "SELECT name, description, type, default, tier \
         FROM system.merge_tree_settings WHERE is_obsolete = 0 ORDER BY name",
        &out_dir.join("merge_tree_settings.json"),
    )
    .await?;

    write_query::<DataTypeInfo>(
        &client,
        "SELECT name, case_insensitive, alias_to \
         FROM system.data_type_families ORDER BY name",
        &out_dir.join("data_types.json"),
    )
    .await?;

    write_query::<TableEngineInfo>(
        &client,
        "SELECT name, supports_settings, supports_sort_order, \
         supports_ttl, supports_replication \
         FROM system.table_engines ORDER BY name",
        &out_dir.join("table_engines.json"),
    )
    .await?;

    write_query::<FormatInfo>(
        &client,
        "SELECT name, is_input, is_output FROM system.formats ORDER BY name",
        &out_dir.join("formats.json"),
    )
    .await?;

    write_query::<CodecInfo>(
        &client,
        "SELECT name FROM system.codecs ORDER BY name",
        &out_dir.join("codecs.json"),
    )
    .await?;

    // Keywords (system.keywords available since CH 24.4)
    match client
        .query_text("SELECT keyword FROM system.keywords ORDER BY keyword")
        .await
    {
        Ok(text) => {
            let keywords: Vec<String> = text.lines().map(|l| l.trim().to_string()).collect();
            let json = serde_json::to_string_pretty(&keywords)?;
            fs::write(out_dir.join("keywords.json"), &json)?;
            eprintln!("  keywords: {} entries", keywords.len());
        }
        Err(e) => {
            eprintln!("  keywords: skipped (system.keywords not available: {e})");
            fs::write(out_dir.join("keywords.json"), "[]")?;
        }
    }

    fs::write(out_dir.join("version.txt"), &version)?;
    eprintln!("Done. Generated files written to {}", out_dir.display());

    Ok(())
}

async fn write_query<T: serde::de::DeserializeOwned + serde::Serialize>(
    client: &ClickHouseClient,
    sql: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let rows: Vec<T> = client.query_json(sql).await?;
    let json = serde_json::to_string_pretty(&rows)?;
    fs::write(path, &json)?;
    eprintln!(
        "  {}: {} entries",
        path.file_name().unwrap().to_string_lossy(),
        rows.len()
    );
    Ok(())
}
