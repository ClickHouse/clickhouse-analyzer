use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::generated;
use super::types::*;

#[cfg(any(feature = "lsp", feature = "codegen"))]
use crate::connection::client::{ClickHouseClient, ConnectionConfig, ConnectionError};

pub type SharedMetadata = Arc<RwLock<MetadataCache>>;

pub struct MetadataCache {
    /// The ClickHouse version the compiled-in defaults came from.
    pub compiled_version: String,

    /// Live connection (Tier 2+3).
    #[cfg(any(feature = "lsp", feature = "codegen"))]
    client: Option<ClickHouseClient>,
    pub server_version: Option<String>,
    pub live_overlay: bool,

    // ===================================================================
    // Global metadata — Tier 1 (compiled-in) or Tier 2 (live overlay)
    // ===================================================================
    pub functions: Vec<FunctionInfo>,
    pub function_index: HashMap<String, usize>,
    pub settings: Vec<SettingInfo>,
    pub merge_tree_settings: Vec<SettingInfo>,
    pub data_types: Vec<DataTypeInfo>,
    pub table_engines: Vec<TableEngineInfo>,
    pub formats: Vec<FormatInfo>,
    pub codecs: Vec<CodecInfo>,
    pub keywords: Vec<String>,

    // ===================================================================
    // Schema metadata — Tier 3 (always runtime-only)
    // ===================================================================
    pub databases: Vec<DatabaseInfo>,
    pub tables: HashMap<String, Vec<TableInfo>>,
    pub columns: HashMap<(String, String), Vec<ColumnInfo>>,
}

impl MetadataCache {
    /// Initialize with compiled-in defaults (Tier 1). Instant, no I/O.
    pub fn from_compiled_defaults() -> Self {
        let functions: Vec<FunctionInfo> =
            serde_json::from_str(generated::FUNCTIONS_JSON)
                .expect("embedded functions.json is corrupt");
        let function_index = functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.to_lowercase(), i))
            .collect();
        let settings: Vec<SettingInfo> =
            serde_json::from_str(generated::SETTINGS_JSON)
                .expect("embedded settings.json is corrupt");
        let merge_tree_settings: Vec<SettingInfo> =
            serde_json::from_str(generated::MERGE_TREE_SETTINGS_JSON)
                .expect("embedded merge_tree_settings.json is corrupt");
        let data_types: Vec<DataTypeInfo> =
            serde_json::from_str(generated::DATA_TYPES_JSON)
                .expect("embedded data_types.json is corrupt");
        let table_engines: Vec<TableEngineInfo> =
            serde_json::from_str(generated::TABLE_ENGINES_JSON)
                .expect("embedded table_engines.json is corrupt");
        let formats: Vec<FormatInfo> =
            serde_json::from_str(generated::FORMATS_JSON)
                .expect("embedded formats.json is corrupt");
        let codecs: Vec<CodecInfo> =
            serde_json::from_str(generated::CODECS_JSON)
                .expect("embedded codecs.json is corrupt");
        let keywords: Vec<String> =
            serde_json::from_str(generated::KEYWORDS_JSON)
                .expect("embedded keywords.json is corrupt");

        Self {
            compiled_version: generated::CLICKHOUSE_VERSION.trim().to_string(),
            #[cfg(any(feature = "lsp", feature = "codegen"))]
            client: None,
            server_version: None,
            live_overlay: false,
            functions,
            function_index,
            settings,
            merge_tree_settings,
            data_types,
            table_engines,
            formats,
            codecs,
            keywords,
            databases: Vec::new(),
            tables: HashMap::new(),
            columns: HashMap::new(),
        }
    }

    /// Look up a function by name (case-insensitive).
    pub fn lookup_function(&self, name: &str) -> Option<&FunctionInfo> {
        self.function_index
            .get(&name.to_lowercase())
            .map(|&i| &self.functions[i])
    }

    /// Rebuild the function index after replacing functions.
    fn rebuild_function_index(&mut self) {
        self.function_index = self
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.to_lowercase(), i))
            .collect();
    }

    pub fn is_connected(&self) -> bool {
        self.live_overlay
    }

    /// Get a reference to the client (for server-side validation).
    #[cfg(any(feature = "lsp", feature = "codegen"))]
    pub fn client_ref(&self) -> Option<&crate::connection::client::ClickHouseClient> {
        self.client.as_ref()
    }
}

// Connection-dependent methods (require reqwest + tokio)
#[cfg(any(feature = "lsp", feature = "codegen"))]
impl MetadataCache {
    /// Connect to a live ClickHouse and upgrade to Tier 2.
    pub async fn connect(&mut self, config: ConnectionConfig) -> Result<(), ConnectionError> {
        let client = ClickHouseClient::new(config)?;
        client.ping().await?;
        let version = client
            .query_text("SELECT version()")
            .await?
            .trim()
            .to_string();
        // Install client so refresh_global can use it, but fully
        // reset to compiled defaults on failure so we never leave
        // stale metadata from a previous connection behind.
        self.client = Some(client);
        if let Err(e) = self.refresh_global().await {
            self.disconnect();
            return Err(e);
        }
        self.server_version = Some(version);
        self.live_overlay = true;
        Ok(())
    }

    /// Disconnect and revert to compiled-in defaults (Tier 1).
    pub fn disconnect(&mut self) {
        *self = Self::from_compiled_defaults();
    }

    /// Refresh global metadata from the live server (Tier 2).
    pub async fn refresh_global(&mut self) -> Result<(), ConnectionError> {
        // Query all system tables concurrently, collecting results.
        // We scope the client borrow so it's dropped before we mutate self.
        let (functions, settings, mt_settings, data_types, engines, formats, codecs, databases, keywords) = {
            let client = self.client.as_ref().ok_or(ConnectionError::NotConnected)?;
            let (functions, settings, mt_settings, data_types, engines, formats, codecs, databases, keywords) =
                tokio::join!(
                    client.query_json::<FunctionInfo>(
                        "SELECT name, is_aggregate, case_insensitive, alias_to, \
                         syntax, arguments, returned_value, description, categories \
                         FROM system.functions ORDER BY name"
                    ),
                    client.query_json::<SettingInfo>(
                        "SELECT name, description, type, default, tier \
                         FROM system.settings WHERE is_obsolete = 0 ORDER BY name"
                    ),
                    client.query_json::<SettingInfo>(
                        "SELECT name, description, type, default, tier \
                         FROM system.merge_tree_settings WHERE is_obsolete = 0 ORDER BY name"
                    ),
                    client.query_json::<DataTypeInfo>(
                        "SELECT name, case_insensitive, alias_to \
                         FROM system.data_type_families ORDER BY name"
                    ),
                    client.query_json::<TableEngineInfo>(
                        "SELECT name, supports_settings, supports_sort_order, \
                         supports_ttl, supports_replication \
                         FROM system.table_engines ORDER BY name"
                    ),
                    client.query_json::<FormatInfo>(
                        "SELECT name, is_input, is_output \
                         FROM system.formats ORDER BY name"
                    ),
                    client.query_json::<CodecInfo>(
                        "SELECT name FROM system.codecs ORDER BY name"
                    ),
                    client.query_json::<DatabaseInfo>(
                        "SELECT name, engine FROM system.databases ORDER BY name"
                    ),
                    client.query_text(
                        "SELECT keyword FROM system.keywords ORDER BY keyword"
                    ),
                );
            (functions, settings, mt_settings, data_types, engines, formats, codecs, databases, keywords)
        };
        // Client borrow dropped — safe to mutate self now.
        // Collect all results before mutating, so a single failure
        // doesn't leave metadata in a partially-updated state.
        let functions = functions?;
        let settings = settings?;
        let mt_settings = mt_settings?;
        let data_types = data_types?;
        let engines = engines?;
        let formats = formats?;
        let codecs = codecs?;
        let databases = databases?;

        self.functions = functions;
        self.rebuild_function_index();
        self.settings = settings;
        self.merge_tree_settings = mt_settings;
        self.data_types = data_types;
        self.table_engines = engines;
        self.formats = formats;
        self.codecs = codecs;
        self.databases = databases;

        // Keywords (system.keywords available since CH 24.4)
        match keywords {
            Ok(text) => {
                self.keywords = text
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.trim().to_string())
                    .collect();
            }
            Err(_) => {
                // Keep existing keywords (from compiled-in defaults)
            }
        }

        Ok(())
    }

    /// Lazy-load tables for a database (Tier 3).
    pub async fn ensure_tables(&mut self, database: &str) -> Result<(), ConnectionError> {
        if self.tables.contains_key(database) {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or(ConnectionError::NotConnected)?;
        let tables: Vec<TableInfo> = client
            .query_json(&format!(
                "SELECT database, name, engine, comment \
                 FROM system.tables WHERE database = '{}' ORDER BY name",
                database.replace('\\', "\\\\").replace('\'', "\\'")
            ))
            .await?;
        self.tables.insert(database.to_string(), tables);
        Ok(())
    }

    /// Get cached tables for a database (call ensure_tables first).
    pub fn get_tables(&self, database: &str) -> &[TableInfo] {
        self.tables
            .get(database)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Lazy-load columns for a table (Tier 3).
    pub async fn ensure_columns(
        &mut self,
        database: &str,
        table: &str,
    ) -> Result<(), ConnectionError> {
        let key = (database.to_string(), table.to_string());
        if self.columns.contains_key(&key) {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or(ConnectionError::NotConnected)?;
        let columns: Vec<ColumnInfo> = client
            .query_json(&format!(
                "SELECT name, type, default_kind, default_expression, comment \
                 FROM system.columns WHERE database = '{}' AND table = '{}' \
                 ORDER BY position",
                database.replace('\\', "\\\\").replace('\'', "\\'"),
                table.replace('\\', "\\\\").replace('\'', "\\'")
            ))
            .await?;
        self.columns.insert(key, columns);
        Ok(())
    }

    /// Get cached columns for a table (call ensure_columns first).
    pub fn get_columns(&self, database: &str, table: &str) -> &[ColumnInfo] {
        let key = (database.to_string(), table.to_string());
        self.columns
            .get(&key)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Invalidate cached schema for a database.
    pub fn invalidate_tables(&mut self, database: &str) {
        self.tables.remove(database);
        let db_owned = database.to_string();
        self.columns
            .retain(|key, _| key.0 != db_owned);
    }

    /// Invalidate all cached schema.
    pub fn invalidate_all_schema(&mut self) {
        self.tables.clear();
        self.columns.clear();
    }

    /// Get the default database from the connection config, or "default".
    pub fn default_database(&self) -> &str {
        self.client
            .as_ref()
            .map(|c| c.config().database.as_str())
            .unwrap_or("default")
    }
}
