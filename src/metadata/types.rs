use serde::{Deserialize, Deserializer, Serialize};

/// ClickHouse returns booleans as 0/1 integers in JSONEachRow format.
fn bool_from_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrInt {
        Bool(bool),
        Int(u8),
    }
    match BoolOrInt::deserialize(deserializer)? {
        BoolOrInt::Bool(b) => Ok(b),
        BoolOrInt::Int(i) => Ok(i != 0),
    }
}

// =======================================================================
// Tier 1/2: Global metadata (compiled-in or from live server)
// =======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub is_aggregate: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub case_insensitive: bool,
    #[serde(default)]
    pub alias_to: String,
    #[serde(default)]
    pub syntax: String,
    #[serde(default)]
    pub arguments: String,
    #[serde(default)]
    pub returned_value: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub categories: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "type")]
    pub value_type: String,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeInfo {
    pub name: String,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub case_insensitive: bool,
    #[serde(default)]
    pub alias_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableEngineInfo {
    pub name: String,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub supports_settings: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub supports_sort_order: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub supports_ttl: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub supports_replication: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub name: String,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub is_input: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub is_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecInfo {
    pub name: String,
}

// =======================================================================
// Tier 3: Schema metadata (always runtime-only)
// =======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    #[serde(default)]
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub database: String,
    pub name: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(default, rename = "type")]
    pub data_type: String,
    #[serde(default)]
    pub default_kind: String,
    #[serde(default)]
    pub default_expression: String,
    #[serde(default)]
    pub comment: String,
}
