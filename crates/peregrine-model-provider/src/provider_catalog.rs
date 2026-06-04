use std::collections::HashMap;

use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_4_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_model_provider_info::WireApi;

pub const ANTHROPIC_PROVIDER_ID: &str = "anthropic";
pub const ANTHROPIC_PROVIDER_NAME: &str = "Anthropic";
pub const ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const ANTHROPIC_API_KEY_ENV_VAR: &str = "ANTHROPIC_API_KEY";
pub const ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-4-6";
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-5.4";
pub const OLLAMA_DEFAULT_MODEL: &str = "gpt-oss:20b";
pub const LMSTUDIO_DEFAULT_MODEL: &str = "openai/gpt-oss-20b";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    AmazonBedrock,
    OpenAiCompatible,
    LocalOpenAiCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderWireApi {
    Responses,
    ChatCompletions,
    AnthropicMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAuthStrategy {
    AccountOrApiKey,
    ApiKey,
    Aws,
    None,
    External,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub kind: ProviderKind,
    pub wire_api: ProviderWireApi,
    pub auth_strategy: ProviderAuthStrategy,
    pub selected: bool,
    pub configured: bool,
    pub selectable: bool,
    pub disabled_reason: Option<String>,
    pub default_model: Option<String>,
}

pub fn provider_catalog_entries(
    selected_provider_id: &str,
    model_providers: &HashMap<String, ModelProviderInfo>,
) -> Vec<ProviderCatalogEntry> {
    let mut entries = Vec::new();
    for id in [
        OPENAI_PROVIDER_ID,
        ANTHROPIC_PROVIDER_ID,
        OLLAMA_OSS_PROVIDER_ID,
        AMAZON_BEDROCK_PROVIDER_ID,
        LMSTUDIO_OSS_PROVIDER_ID,
    ] {
        entries.push(provider_catalog_entry_for_id(
            selected_provider_id,
            model_providers,
            id,
        ));
    }

    let mut custom_ids = model_providers
        .keys()
        .filter(|id| {
            !matches!(
                id.as_str(),
                OPENAI_PROVIDER_ID
                    | ANTHROPIC_PROVIDER_ID
                    | OLLAMA_OSS_PROVIDER_ID
                    | LMSTUDIO_OSS_PROVIDER_ID
                    | AMAZON_BEDROCK_PROVIDER_ID
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    custom_ids.sort();
    entries.extend(custom_ids.into_iter().map(|id| {
        provider_catalog_entry_for_id(selected_provider_id, model_providers, id.as_str())
    }));
    entries
}

pub fn add_peregrine_builtin_model_providers(
    mut providers: HashMap<String, ModelProviderInfo>,
) -> HashMap<String, ModelProviderInfo> {
    providers
        .entry(ANTHROPIC_PROVIDER_ID.to_string())
        .or_insert_with(create_anthropic_provider);
    providers
}

pub fn create_anthropic_provider() -> ModelProviderInfo {
    let http_headers = HashMap::from([("anthropic-version".to_string(), "2023-06-01".to_string())]);
    let env_http_headers = HashMap::from([(
        "x-api-key".to_string(),
        ANTHROPIC_API_KEY_ENV_VAR.to_string(),
    )]);

    ModelProviderInfo {
        name: ANTHROPIC_PROVIDER_NAME.to_string(),
        base_url: Some(ANTHROPIC_DEFAULT_BASE_URL.to_string()),
        env_key: Some(ANTHROPIC_API_KEY_ENV_VAR.to_string()),
        env_key_instructions: Some("Set ANTHROPIC_API_KEY to an Anthropic API key.".to_string()),
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        // The upstream Codex metadata crate currently serializes only
        // `responses`. Peregrine maps this built-in provider to Anthropic
        // Messages at runtime through `ModelProvider::runtime_wire_api`.
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: Some(http_headers),
        env_http_headers: Some(env_http_headers),
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
    }
}

pub fn is_anthropic_provider_info(provider: &ModelProviderInfo) -> bool {
    provider.name == ANTHROPIC_PROVIDER_NAME
        && provider
            .base_url
            .as_deref()
            .is_some_and(|base_url| base_url.contains("anthropic.com"))
}

pub fn provider_catalog_entry_for_id(
    selected_provider_id: &str,
    model_providers: &HashMap<String, ModelProviderInfo>,
    id: &str,
) -> ProviderCatalogEntry {
    let provider = model_providers.get(id);
    let configured = provider.is_some();
    let selected = selected_provider_id == id;
    let kind = provider_kind(id);
    let auth_strategy = provider_auth_strategy(id, provider);
    let display_name = provider
        .map(|provider| provider_display_name(id, provider.name.as_str()))
        .unwrap_or_else(|| missing_provider_display_name(id));
    let description = provider_description(id);
    let disabled_reason = provider_disabled_reason(configured);
    let selectable = disabled_reason.is_none();

    ProviderCatalogEntry {
        id: id.to_string(),
        display_name,
        description,
        kind,
        wire_api: provider_wire_api(id),
        auth_strategy,
        selected,
        configured,
        selectable,
        disabled_reason,
        default_model: default_model_for_provider(id).map(str::to_string),
    }
}

pub fn default_model_for_provider(id: &str) -> Option<&'static str> {
    match id {
        OPENAI_PROVIDER_ID => Some(OPENAI_DEFAULT_MODEL),
        ANTHROPIC_PROVIDER_ID => Some(ANTHROPIC_DEFAULT_MODEL),
        OLLAMA_OSS_PROVIDER_ID => Some(OLLAMA_DEFAULT_MODEL),
        LMSTUDIO_OSS_PROVIDER_ID => Some(LMSTUDIO_DEFAULT_MODEL),
        AMAZON_BEDROCK_PROVIDER_ID => Some(AMAZON_BEDROCK_GPT_5_4_MODEL_ID),
        _ => None,
    }
}

fn provider_kind(id: &str) -> ProviderKind {
    match id {
        OPENAI_PROVIDER_ID => ProviderKind::OpenAi,
        ANTHROPIC_PROVIDER_ID => ProviderKind::Anthropic,
        OLLAMA_OSS_PROVIDER_ID => ProviderKind::Ollama,
        AMAZON_BEDROCK_PROVIDER_ID => ProviderKind::AmazonBedrock,
        LMSTUDIO_OSS_PROVIDER_ID => ProviderKind::LocalOpenAiCompatible,
        _ => ProviderKind::OpenAiCompatible,
    }
}

fn provider_wire_api(id: &str) -> ProviderWireApi {
    match id {
        ANTHROPIC_PROVIDER_ID => ProviderWireApi::AnthropicMessages,
        _ => ProviderWireApi::Responses,
    }
}

fn provider_auth_strategy(id: &str, provider: Option<&ModelProviderInfo>) -> ProviderAuthStrategy {
    match id {
        OPENAI_PROVIDER_ID => ProviderAuthStrategy::AccountOrApiKey,
        ANTHROPIC_PROVIDER_ID => ProviderAuthStrategy::ApiKey,
        OLLAMA_OSS_PROVIDER_ID | LMSTUDIO_OSS_PROVIDER_ID => ProviderAuthStrategy::None,
        AMAZON_BEDROCK_PROVIDER_ID => ProviderAuthStrategy::Aws,
        _ => provider.map_or(ProviderAuthStrategy::Unsupported, |provider| {
            if provider.aws.is_some() {
                ProviderAuthStrategy::Aws
            } else if provider.auth.is_some() {
                ProviderAuthStrategy::External
            } else if provider.env_key.is_some() || provider.experimental_bearer_token.is_some() {
                ProviderAuthStrategy::ApiKey
            } else if provider.requires_openai_auth {
                ProviderAuthStrategy::AccountOrApiKey
            } else {
                ProviderAuthStrategy::None
            }
        }),
    }
}

fn provider_display_name(id: &str, configured_name: &str) -> String {
    match id {
        OPENAI_PROVIDER_ID => "OpenAI".to_string(),
        ANTHROPIC_PROVIDER_ID => "Anthropic".to_string(),
        OLLAMA_OSS_PROVIDER_ID => "Ollama".to_string(),
        AMAZON_BEDROCK_PROVIDER_ID => "Amazon Bedrock".to_string(),
        LMSTUDIO_OSS_PROVIDER_ID => "LM Studio".to_string(),
        _ if configured_name.trim().is_empty() => id.to_string(),
        _ => configured_name.to_string(),
    }
}

fn missing_provider_display_name(id: &str) -> String {
    match id {
        ANTHROPIC_PROVIDER_ID => "Anthropic".to_string(),
        _ => id.to_string(),
    }
}

fn provider_description(id: &str) -> String {
    match id {
        OPENAI_PROVIDER_ID => "Use OpenAI models with browser account auth or an API key.",
        ANTHROPIC_PROVIDER_ID => "Use Anthropic Claude through the native Messages API.",
        OLLAMA_OSS_PROVIDER_ID => "Use local models served by Ollama's OpenAI-compatible API.",
        AMAZON_BEDROCK_PROVIDER_ID => {
            "Use OpenAI models through Amazon Bedrock Mantle with AWS auth."
        }
        LMSTUDIO_OSS_PROVIDER_ID => "Use local models served by LM Studio's OpenAI-compatible API.",
        _ => "Use a configured OpenAI Responses-compatible provider.",
    }
    .to_string()
}

fn provider_disabled_reason(configured: bool) -> Option<String> {
    if configured {
        return None;
    }

    Some("provider is not configured".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::built_in_model_providers;

    #[test]
    fn built_in_catalog_exposes_bedrock_as_selectable_aws_provider() {
        let providers = built_in_model_providers(/*openai_base_url*/ None);

        let entry = provider_catalog_entry_for_id(
            OPENAI_PROVIDER_ID,
            &providers,
            AMAZON_BEDROCK_PROVIDER_ID,
        );

        assert_eq!(entry.kind, ProviderKind::AmazonBedrock);
        assert_eq!(entry.wire_api, ProviderWireApi::Responses);
        assert_eq!(entry.auth_strategy, ProviderAuthStrategy::Aws);
        assert!(entry.selectable);
        assert_eq!(
            entry.default_model.as_deref(),
            Some(AMAZON_BEDROCK_GPT_5_4_MODEL_ID)
        );
    }

    #[test]
    fn anthropic_is_first_party_messages_provider() {
        let providers = add_peregrine_builtin_model_providers(built_in_model_providers(
            /*openai_base_url*/ None,
        ));

        let entry =
            provider_catalog_entry_for_id(OPENAI_PROVIDER_ID, &providers, ANTHROPIC_PROVIDER_ID);

        assert_eq!(entry.kind, ProviderKind::Anthropic);
        assert_eq!(entry.wire_api, ProviderWireApi::AnthropicMessages);
        assert_eq!(entry.auth_strategy, ProviderAuthStrategy::ApiKey);
        assert!(entry.configured);
        assert!(entry.selectable);
        assert_eq!(
            entry.default_model.as_deref(),
            Some(ANTHROPIC_DEFAULT_MODEL)
        );
    }

    #[test]
    fn custom_provider_is_openai_responses_compatible() {
        let mut providers = built_in_model_providers(/*openai_base_url*/ None);
        providers.insert(
            "custom".to_string(),
            ModelProviderInfo {
                name: "Custom".to_string(),
                base_url: Some("https://example.test/v1".to_string()),
                ..ModelProviderInfo::default()
            },
        );

        let entry = provider_catalog_entry_for_id("custom", &providers, "custom");

        assert_eq!(entry.kind, ProviderKind::OpenAiCompatible);
        assert_eq!(entry.wire_api, ProviderWireApi::Responses);
        assert_eq!(entry.display_name, "Custom");
        assert!(entry.configured);
        assert!(entry.selectable);
    }
}
