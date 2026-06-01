use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use peregrine_core::config::Config;
use serde::Deserialize;

const WORKSPACE_SETTINGS_TIMEOUT: Duration = Duration::from_secs(10);
const WORKSPACE_SETTINGS_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const PEREGRINE_PLUGINS_BETA_SETTING: &str = "enable_plugins";
const OAI_PRODUCT_SKU_HEADER: &str = "OAI-Product-Sku";
const CODEX_PRODUCT_SKU: &str = "codex";

#[derive(Debug, Deserialize)]
struct WorkspaceSettingsResponse {
    #[serde(default)]
    beta_settings: HashMap<String, bool>,
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceSettingsCache {
    entry: RwLock<Option<CachedWorkspaceSettings>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WorkspaceSettingsCacheKey {
    chatgpt_base_url: String,
    account_id: String,
}

#[derive(Clone, Debug)]
struct CachedWorkspaceSettings {
    key: WorkspaceSettingsCacheKey,
    expires_at: Instant,
    peregrine_plugins_enabled: bool,
}

impl WorkspaceSettingsCache {
    fn get_peregrine_plugins_enabled(&self, key: &WorkspaceSettingsCacheKey) -> Option<bool> {
        {
            let entry = match self.entry.read() {
                Ok(entry) => entry,
                Err(err) => err.into_inner(),
            };
            let now = Instant::now();
            if let Some(cached) = entry.as_ref()
                && now < cached.expires_at
                && cached.key == *key
            {
                return Some(cached.peregrine_plugins_enabled);
            }
        }

        let mut entry = match self.entry.write() {
            Ok(entry) => entry,
            Err(err) => err.into_inner(),
        };
        let now = Instant::now();
        if entry
            .as_ref()
            .is_some_and(|cached| now >= cached.expires_at || cached.key != *key)
        {
            *entry = None;
        }
        None
    }

    fn set_peregrine_plugins_enabled(&self, key: WorkspaceSettingsCacheKey, enabled: bool) {
        let mut entry = match self.entry.write() {
            Ok(entry) => entry,
            Err(err) => err.into_inner(),
        };
        *entry = Some(CachedWorkspaceSettings {
            key,
            expires_at: Instant::now() + WORKSPACE_SETTINGS_CACHE_TTL,
            peregrine_plugins_enabled: enabled,
        });
    }
}

pub(crate) async fn peregrine_plugins_enabled_for_workspace(
    config: &Config,
    auth: Option<&CodexAuth>,
    cache: Option<&WorkspaceSettingsCache>,
) -> anyhow::Result<bool> {
    let Some(auth) = auth else {
        return Ok(true);
    };
    if !auth.is_chatgpt_auth() {
        return Ok(true);
    }

    let token_data = auth
        .get_token_data()
        .context("ChatGPT token data is not available")?;
    if !token_data.id_token.is_workspace_account() {
        return Ok(true);
    }

    let Some(account_id) = token_data.account_id.as_deref().filter(|id| !id.is_empty()) else {
        return Ok(true);
    };

    let cache_key = WorkspaceSettingsCacheKey {
        chatgpt_base_url: config.chatgpt_base_url.clone(),
        account_id: account_id.to_string(),
    };
    if let Some(cache) = cache
        && let Some(enabled) = cache.get_peregrine_plugins_enabled(&cache_key)
    {
        return Ok(enabled);
    }

    let encoded_account_id = encode_path_segment(account_id);
    let settings: WorkspaceSettingsResponse = chatgpt_get_request_with_timeout(
        config,
        auth,
        format!("/accounts/{encoded_account_id}/settings"),
        WORKSPACE_SETTINGS_TIMEOUT,
    )
    .await?;

    let peregrine_plugins_enabled = settings
        .beta_settings
        .get(PEREGRINE_PLUGINS_BETA_SETTING)
        .copied()
        .unwrap_or(true);

    if let Some(cache) = cache {
        cache.set_peregrine_plugins_enabled(cache_key, peregrine_plugins_enabled);
    }

    Ok(peregrine_plugins_enabled)
}

async fn chatgpt_get_request_with_timeout<T: serde::de::DeserializeOwned>(
    config: &Config,
    auth: &CodexAuth,
    path: String,
    timeout: Duration,
) -> anyhow::Result<T> {
    let client = create_client();
    let url = format!(
        "{}/{}",
        config.chatgpt_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let response = client
        .get(&url)
        .headers(peregrine_model_provider::auth_provider_from_auth(auth).to_auth_headers())
        .header(OAI_PRODUCT_SKU_HEADER, CODEX_PRODUCT_SKU)
        .header("Content-Type", "application/json")
        .timeout(timeout)
        .send()
        .await
        .context("Failed to send request")?;

    if response.status().is_success() {
        response
            .json()
            .await
            .context("Failed to parse JSON response")
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Request failed with status {status}: {body}")
    }
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}
