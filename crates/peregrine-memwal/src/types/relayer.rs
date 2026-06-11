use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct MinSupportedSdk {
    pub typescript: String,
    pub python: String,
    pub mcp: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayerDeprecationNotice {
    pub surface: String,
    pub deprecated_since: String,
    pub removal_api_version: String,
    pub guidance: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RelayerBuildMetadata {
    pub commit: Option<String>,
    pub build_timestamp: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayerVersionMetadata {
    #[serde(rename = "relayerVersion")]
    pub relayer_version: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(rename = "minSupportedSdk")]
    pub min_supported_sdk: MinSupportedSdk,
    #[serde(rename = "featureFlags")]
    pub feature_flags: std::collections::BTreeMap<String, bool>,
    pub deprecations: Vec<RelayerDeprecationNotice>,
    pub build: RelayerBuildMetadata,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HealthResult {
    pub status: String,
    pub version: String,
    #[serde(rename = "relayerVersion")]
    pub relayer_version: Option<String>,
    #[serde(rename = "apiVersion")]
    pub api_version: Option<String>,
    #[serde(rename = "minSupportedSdk")]
    pub min_supported_sdk: Option<MinSupportedSdk>,
    #[serde(rename = "featureFlags")]
    pub feature_flags: Option<std::collections::BTreeMap<String, bool>>,
    pub deprecations: Option<Vec<RelayerDeprecationNotice>>,
    pub build: Option<RelayerBuildMetadata>,
    pub mode: Option<String>,
    pub prompt_versions: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayerConfig {
    #[serde(rename = "packageId")]
    pub package_id: sui_sdk_types::Address,
    pub network: String,
    #[serde(rename = "suiRpcUrl")]
    pub sui_rpc_url: String,
}
