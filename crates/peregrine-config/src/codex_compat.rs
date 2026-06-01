use std::collections::BTreeMap;
use std::collections::HashMap;

use peregrine_types::protocol::AskForApproval;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub type AppToolApproval = codex_config::AppToolApproval;
pub type ApprovalPolicy = codex_config::Constrained<AskForApproval>;
pub type ConfigLayerStack = codex_config::ConfigLayerStack;
pub type McpServerConfig = codex_config::McpServerConfig;
pub type McpServerTransportConfig = codex_config::McpServerTransportConfig;
pub type OAuthCredentialsStoreMode = codex_config::types::OAuthCredentialsStoreMode;

pub fn config_layer_stack_from_effective_config(effective_config: toml::Value) -> ConfigLayerStack {
    let layer = codex_config::ConfigLayerEntry::new(
        codex_config::ConfigLayerSource::SessionFlags,
        effective_config,
    );
    codex_config::ConfigLayerStack::new(
        vec![layer],
        codex_config::ConfigRequirements::default(),
        codex_config::ConfigRequirementsToml::default(),
    )
    .unwrap_or_default()
}

pub fn app_tool_approval(mode: crate::AppToolApproval) -> AppToolApproval {
    match mode {
        crate::AppToolApproval::Auto => codex_config::AppToolApproval::Auto,
        crate::AppToolApproval::Prompt => codex_config::AppToolApproval::Prompt,
        crate::AppToolApproval::Approve => codex_config::AppToolApproval::Approve,
    }
}

pub fn oauth_credentials_store_mode(
    mode: crate::types::OAuthCredentialsStoreMode,
) -> OAuthCredentialsStoreMode {
    match mode {
        crate::types::OAuthCredentialsStoreMode::Auto => {
            codex_config::types::OAuthCredentialsStoreMode::Auto
        }
        crate::types::OAuthCredentialsStoreMode::File => {
            codex_config::types::OAuthCredentialsStoreMode::File
        }
        crate::types::OAuthCredentialsStoreMode::Keyring => {
            codex_config::types::OAuthCredentialsStoreMode::Keyring
        }
    }
}

pub fn approval_policy(policy: &crate::Constrained<AskForApproval>) -> ApprovalPolicy {
    codex_config::Constrained::allow_any(policy.value())
}

pub fn mcp_server_config_map_to_codex<T>(servers: &T) -> HashMap<String, McpServerConfig>
where
    T: Serialize,
{
    serde_convert(servers).unwrap_or_default()
}

pub fn mcp_server_config_map_from_codex(
    servers: &HashMap<String, McpServerConfig>,
) -> HashMap<String, crate::McpServerConfig> {
    serde_convert(servers).unwrap_or_default()
}

pub fn mcp_server_config_btree_from_codex(
    servers: &HashMap<String, McpServerConfig>,
) -> BTreeMap<String, crate::McpServerConfig> {
    serde_convert(servers).unwrap_or_default()
}

fn serde_convert<T, U>(value: &T) -> Option<U>
where
    T: Serialize,
    U: DeserializeOwned,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
}
