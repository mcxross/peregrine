#![allow(clippy::unwrap_used, clippy::expect_used, clippy::match_like_matches_macro, clippy::redundant_clone, clippy::too_many_arguments, clippy::result_large_err)]
use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_4_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use peregrine_app_server_protocol::JSONRPCResponse;
use peregrine_app_server_protocol::ModelProviderAuthStrategy;
use peregrine_app_server_protocol::ModelProviderCredentialState;
use peregrine_app_server_protocol::ModelProviderEntry;
use peregrine_app_server_protocol::ModelProviderKind;
use peregrine_app_server_protocol::ModelProviderListParams;
use peregrine_app_server_protocol::ModelProviderListResponse;
use peregrine_app_server_protocol::ModelProviderModelsListParams;
use peregrine_app_server_protocol::ModelProviderModelsListResponse;
use peregrine_app_server_protocol::ModelProviderSelectParams;
use peregrine_app_server_protocol::ModelProviderSelectResponse;
use peregrine_app_server_protocol::ModelProviderWireApi;
use peregrine_app_server_protocol::RequestId;
use peregrine_model_provider::ANTHROPIC_API_KEY_ENV_VAR;
use peregrine_model_provider::ANTHROPIC_DEFAULT_MODEL;
use peregrine_model_provider::ANTHROPIC_PROVIDER_ID;
use peregrine_model_provider::LMSTUDIO_DEFAULT_MODEL;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn list_exposes_first_party_provider_catalog() -> Result<()> {
    let peregrine_home = TempDir::new()?;
    let mut mcp = McpProcess::new_with_env(
        peregrine_home.path(),
        &[("OPENAI_API_KEY", None), (ANTHROPIC_API_KEY_ENV_VAR, None)],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let response = model_provider_list(&mut mcp).await?;

    assert_eq!(response.selected_provider_id, OPENAI_PROVIDER_ID);

    let openai = provider_by_id(&response, OPENAI_PROVIDER_ID);
    assert_eq!(openai.display_name, "OpenAI");
    assert_eq!(openai.kind, ModelProviderKind::OpenAi);
    assert_eq!(openai.wire_api, ModelProviderWireApi::Responses);
    assert_eq!(
        openai.auth_strategy,
        ModelProviderAuthStrategy::AccountOrApiKey
    );
    assert_eq!(
        openai.credential_state,
        Some(ModelProviderCredentialState::NeedsLogin)
    );
    assert_eq!(
        openai.setup_hint.as_deref(),
        Some("Use /login or set OPENAI_API_KEY before starting a turn.")
    );
    assert!(openai.selectable);
    assert!(openai.selected);

    let ollama = provider_by_id(&response, OLLAMA_OSS_PROVIDER_ID);
    assert_eq!(ollama.display_name, "Ollama");
    assert_eq!(ollama.kind, ModelProviderKind::Ollama);
    assert_eq!(ollama.auth_strategy, ModelProviderAuthStrategy::None);
    assert_eq!(
        ollama.credential_state,
        Some(ModelProviderCredentialState::NotRequired)
    );
    assert_eq!(ollama.setup_hint, None);
    assert!(ollama.selectable);

    let bedrock = provider_by_id(&response, AMAZON_BEDROCK_PROVIDER_ID);
    assert_eq!(bedrock.display_name, "Amazon Bedrock");
    assert_eq!(bedrock.kind, ModelProviderKind::AmazonBedrock);
    assert_eq!(bedrock.auth_strategy, ModelProviderAuthStrategy::Aws);
    assert_eq!(
        bedrock.credential_state,
        Some(ModelProviderCredentialState::Unknown)
    );
    assert_eq!(bedrock.setup_hint, None);
    assert_eq!(
        bedrock.default_model.as_deref(),
        Some(AMAZON_BEDROCK_GPT_5_4_MODEL_ID)
    );
    assert!(bedrock.selectable);

    let anthropic = provider_by_id(&response, ANTHROPIC_PROVIDER_ID);
    assert_eq!(anthropic.display_name, "Anthropic");
    assert_eq!(anthropic.kind, ModelProviderKind::Anthropic);
    assert_eq!(anthropic.wire_api, ModelProviderWireApi::AnthropicMessages);
    assert_eq!(anthropic.auth_strategy, ModelProviderAuthStrategy::ApiKey);
    assert_eq!(
        anthropic.credential_state,
        Some(ModelProviderCredentialState::MissingApiKey)
    );
    assert_eq!(
        anthropic.setup_hint.as_deref(),
        Some("Set ANTHROPIC_API_KEY before starting a turn.")
    );
    assert_eq!(
        anthropic.default_model.as_deref(),
        Some(ANTHROPIC_DEFAULT_MODEL)
    );
    assert!(anthropic.selectable);

    let lmstudio = provider_by_id(&response, LMSTUDIO_OSS_PROVIDER_ID);
    assert_eq!(lmstudio.display_name, "LM Studio");
    assert_eq!(lmstudio.kind, ModelProviderKind::LocalOpenAiCompatible);
    assert_eq!(lmstudio.auth_strategy, ModelProviderAuthStrategy::None);
    assert_eq!(
        lmstudio.default_model.as_deref(),
        Some(LMSTUDIO_DEFAULT_MODEL)
    );
    assert_eq!(
        lmstudio.credential_state,
        Some(ModelProviderCredentialState::NotRequired)
    );
    assert_eq!(lmstudio.setup_hint, None);
    assert!(lmstudio.selectable);

    Ok(())
}

#[tokio::test]
async fn list_marks_anthropic_ready_when_env_key_present() -> Result<()> {
    let peregrine_home = TempDir::new()?;
    let mut mcp = McpProcess::new_with_env(
        peregrine_home.path(),
        &[
            (ANTHROPIC_API_KEY_ENV_VAR, Some("test-anthropic-key")),
            ("OPENAI_API_KEY", None),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let response = model_provider_list(&mut mcp).await?;
    let anthropic = provider_by_id(&response, ANTHROPIC_PROVIDER_ID);
    assert_eq!(
        anthropic.credential_state,
        Some(ModelProviderCredentialState::Ready)
    );
    assert_eq!(anthropic.setup_hint, None);

    Ok(())
}

#[tokio::test]
async fn select_bedrock_persists_provider_and_default_model() -> Result<()> {
    let peregrine_home = TempDir::new()?;
    let mut mcp = McpProcess::new(peregrine_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_model_provider_select_request(ModelProviderSelectParams {
            provider_id: AMAZON_BEDROCK_PROVIDER_ID.to_string(),
            model: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: ModelProviderSelectResponse = to_response(response)?;

    assert_eq!(received.provider.id, AMAZON_BEDROCK_PROVIDER_ID);
    assert_eq!(received.selected_provider.id, AMAZON_BEDROCK_PROVIDER_ID);
    assert_eq!(
        received.model.as_deref(),
        Some(AMAZON_BEDROCK_GPT_5_4_MODEL_ID)
    );

    let config_toml = std::fs::read_to_string(peregrine_home.path().join("config.toml"))?;
    assert!(config_toml.contains(r#"model_provider = "amazon-bedrock""#));
    assert!(config_toml.contains(r#"model = "openai.gpt-5.4""#));

    let response = model_provider_list(&mut mcp).await?;
    let bedrock = provider_by_id(&response, AMAZON_BEDROCK_PROVIDER_ID);
    assert!(bedrock.selected);

    Ok(())
}

#[tokio::test]
async fn select_anthropic_persists_provider_and_default_model() -> Result<()> {
    let peregrine_home = TempDir::new()?;
    let mut mcp =
        McpProcess::new_with_env(peregrine_home.path(), &[(ANTHROPIC_API_KEY_ENV_VAR, None)])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_model_provider_select_request(ModelProviderSelectParams {
            provider_id: ANTHROPIC_PROVIDER_ID.to_string(),
            model: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: ModelProviderSelectResponse = to_response(response)?;

    assert_eq!(received.provider.id, ANTHROPIC_PROVIDER_ID);
    assert_eq!(
        received.provider.credential_state,
        Some(ModelProviderCredentialState::MissingApiKey)
    );
    assert_eq!(
        received.provider.setup_hint.as_deref(),
        Some("Set ANTHROPIC_API_KEY before starting a turn.")
    );
    assert_eq!(received.selected_provider.id, ANTHROPIC_PROVIDER_ID);
    assert_eq!(received.model.as_deref(), Some(ANTHROPIC_DEFAULT_MODEL));

    let config_toml = std::fs::read_to_string(peregrine_home.path().join("config.toml"))?;
    assert!(config_toml.contains(r#"model_provider = "anthropic""#));
    assert!(config_toml.contains(&format!(r#"model = "{ANTHROPIC_DEFAULT_MODEL}""#)));

    let response = model_provider_list(&mut mcp).await?;
    let anthropic = provider_by_id(&response, ANTHROPIC_PROVIDER_ID);
    assert!(anthropic.selected);

    Ok(())
}

#[tokio::test]
async fn anthropic_model_list_returns_default_model_without_network_probe() -> Result<()> {
    let peregrine_home = TempDir::new()?;
    let mut mcp = McpProcess::new(peregrine_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_model_provider_models_list_request(ModelProviderModelsListParams {
            provider_id: ANTHROPIC_PROVIDER_ID.to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: ModelProviderModelsListResponse = to_response(response)?;

    assert_eq!(received.provider_id, ANTHROPIC_PROVIDER_ID);
    assert_eq!(received.data.len(), 1);
    assert_eq!(received.data[0].model, ANTHROPIC_DEFAULT_MODEL);
    assert!(received.data[0].is_default);

    Ok(())
}

#[tokio::test]
async fn bedrock_model_list_returns_default_model_without_ollama_network_probe() -> Result<()> {
    let peregrine_home = TempDir::new()?;
    let mut mcp = McpProcess::new(peregrine_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_model_provider_models_list_request(ModelProviderModelsListParams {
            provider_id: AMAZON_BEDROCK_PROVIDER_ID.to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: ModelProviderModelsListResponse = to_response(response)?;

    assert_eq!(received.provider_id, AMAZON_BEDROCK_PROVIDER_ID);
    assert_eq!(received.data.len(), 1);
    assert_eq!(received.data[0].model, AMAZON_BEDROCK_GPT_5_4_MODEL_ID);
    assert!(received.data[0].is_default);

    Ok(())
}

async fn model_provider_list(mcp: &mut McpProcess) -> Result<ModelProviderListResponse> {
    let request_id = mcp
        .send_model_provider_list_request(ModelProviderListParams {})
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn provider_by_id<'a>(response: &'a ModelProviderListResponse, id: &str) -> &'a ModelProviderEntry {
    response
        .data
        .iter()
        .find(|provider| provider.id == id)
        .unwrap_or_else(|| panic!("missing provider `{id}` in {:?}", response.data))
}
