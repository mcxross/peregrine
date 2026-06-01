use std::collections::BTreeMap;
use std::collections::HashMap;

use peregrine_types::protocol::AskForApproval;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub type AppToolApproval = codex_config::AppToolApproval;
pub type ApprovalPolicy = codex_config::Constrained<AskForApproval>;
pub type ConfigLayerStack = codex_config::ConfigLayerStack;
pub type HookEventsToml = codex_config::HookEventsToml;
pub type HooksFile = codex_config::HooksFile;
pub type McpServerConfig = codex_config::McpServerConfig;
pub type McpServerDisabledReason = codex_config::McpServerDisabledReason;
pub type McpServerTransportConfig = codex_config::McpServerTransportConfig;
pub type OAuthCredentialsStoreMode = codex_config::types::OAuthCredentialsStoreMode;
pub type RequirementSource = codex_config::RequirementSource;

pub fn config_layer_stack(stack: &crate::ConfigLayerStack) -> ConfigLayerStack {
    let ignore_user_and_project_exec_policy_rules =
        stack.ignore_user_and_project_exec_policy_rules();
    let layers = stack
        .get_layers(
            crate::ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ true,
        )
        .into_iter()
        .map(config_layer_entry)
        .collect();
    ConfigLayerStack::new(
        layers,
        codex_config::ConfigRequirements::default(),
        codex_config::ConfigRequirementsToml::default(),
    )
    .map(|compat_stack| {
        compat_stack.with_user_and_project_exec_policy_rules_ignored(
            ignore_user_and_project_exec_policy_rules,
        )
    })
    .unwrap_or_default()
}

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

fn config_layer_entry(layer: &crate::ConfigLayerEntry) -> codex_config::ConfigLayerEntry {
    let source = config_layer_source(&layer.name);
    if let Some(disabled_reason) = &layer.disabled_reason {
        codex_config::ConfigLayerEntry::new_disabled(
            source,
            layer.config.clone(),
            disabled_reason.clone(),
        )
    } else if let Some(raw_toml) = &layer.raw_toml {
        codex_config::ConfigLayerEntry::new_with_raw_toml(
            source,
            layer.config.clone(),
            raw_toml.clone(),
        )
    } else {
        codex_config::ConfigLayerEntry::new(source, layer.config.clone())
    }
}

fn config_layer_source(source: &crate::ConfigLayerSource) -> codex_config::ConfigLayerSource {
    match source {
        crate::ConfigLayerSource::Mdm { domain, key } => codex_config::ConfigLayerSource::Mdm {
            domain: domain.clone(),
            key: key.clone(),
        },
        crate::ConfigLayerSource::System { file } => {
            codex_config::ConfigLayerSource::System { file: file.clone() }
        }
        crate::ConfigLayerSource::User { file, profile } => codex_config::ConfigLayerSource::User {
            file: file.clone(),
            profile: profile.clone(),
        },
        crate::ConfigLayerSource::Project {
            dot_peregrine_folder,
        } => codex_config::ConfigLayerSource::Project {
            dot_codex_folder: dot_peregrine_folder.clone(),
        },
        crate::ConfigLayerSource::SessionFlags => codex_config::ConfigLayerSource::SessionFlags,
        crate::ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => {
            codex_config::ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: file.clone() }
        }
        crate::ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {
            codex_config::ConfigLayerSource::LegacyManagedConfigTomlFromMdm
        }
    }
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
