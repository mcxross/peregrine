use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildMetadata {
    pub move_toml_hash: Option<String>,
    pub source_hash: Option<String>,
    pub dependency_metadata_hash: Option<String>,
    pub compiler_version: Option<String>,
    pub sui_framework_version: Option<String>,
    pub indexer_version: String,
    pub extraction_config_hash: Option<String>,
}
