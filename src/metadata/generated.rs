//! Auto-generated ClickHouse metadata compiled into the binary.
//!
//! Do not edit manually. Run `cargo run --bin codegen --features codegen` to regenerate
//! from a live ClickHouse instance.

/// The ClickHouse version these definitions were generated from.
pub const CLICKHOUSE_VERSION: &str = include_str!("../../generated/version.txt");

pub const FUNCTIONS_JSON: &str = include_str!("../../generated/functions.json");
pub const SETTINGS_JSON: &str = include_str!("../../generated/settings.json");
pub const MERGE_TREE_SETTINGS_JSON: &str = include_str!("../../generated/merge_tree_settings.json");
pub const DATA_TYPES_JSON: &str = include_str!("../../generated/data_types.json");
pub const TABLE_ENGINES_JSON: &str = include_str!("../../generated/table_engines.json");
pub const FORMATS_JSON: &str = include_str!("../../generated/formats.json");
pub const CODECS_JSON: &str = include_str!("../../generated/codecs.json");
pub const KEYWORDS_JSON: &str = include_str!("../../generated/keywords.json");
