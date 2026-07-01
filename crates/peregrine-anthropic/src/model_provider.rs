use std::path::PathBuf;
use std::sync::Arc;

use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::Result;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use peregrine_api::Provider;
use peregrine_api::SharedAuthProvider;
use peregrine_model_provider::ANTHROPIC_DEFAULT_MODEL;
use peregrine_model_provider::ModelProvider;
use peregrine_model_provider::ProviderAccountResult;
use peregrine_model_provider::ProviderAccountState;
use peregrine_model_provider::ProviderCapabilities;
use peregrine_model_provider::RuntimeWireApi;
use peregrine_model_provider::api_provider_from_info;
use peregrine_model_provider::unauthenticated_auth_provider;
use peregrine_models_manager::manager::SharedModelsManager;
use peregrine_models_manager::manager::StaticModelsManager;
use peregrine_models_manager::model_info::model_info_from_slug;

/// Runtime model provider for Anthropic's native Messages API.
#[derive(Clone, Debug)]
pub struct AnthropicModelProvider {
    info: ModelProviderInfo,
}

impl AnthropicModelProvider {
    pub fn new(provider_info: ModelProviderInfo) -> Self {
        Self {
            info: provider_info,
        }
    }
}

#[async_trait::async_trait]
impl ModelProvider for AnthropicModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn runtime_wire_api(&self) -> RuntimeWireApi {
        RuntimeWireApi::AnthropicMessages
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            namespace_tools: true,
            image_generation: false,
            web_search: false,
        }
    }

    fn approval_review_preferred_model(&self) -> &'static str {
        ANTHROPIC_DEFAULT_MODEL
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        None
    }

    async fn auth(&self) -> Option<CodexAuth> {
        None
    }

    fn account_state(&self) -> ProviderAccountResult {
        Ok(ProviderAccountState {
            account: None,
            requires_openai_auth: false,
        })
    }

    async fn api_provider(&self) -> Result<Provider> {
        let _ = self.info.api_key()?;
        api_provider_from_info(&self.info, /*auth_mode*/ None)
    }

    async fn api_auth(&self) -> Result<SharedAuthProvider> {
        Ok(unauthenticated_auth_provider())
    }

    fn models_manager(
        &self,
        _codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        Arc::new(StaticModelsManager::new(
            /*auth_manager*/ None,
            config_model_catalog.unwrap_or_else(anthropic_static_model_catalog),
        ))
    }
}

fn anthropic_static_model_catalog() -> ModelsResponse {
    ModelsResponse {
        models: vec![anthropic_model_info(ANTHROPIC_DEFAULT_MODEL)],
    }
}

fn anthropic_model_info(slug: &str) -> ModelInfo {
    let mut model = model_info_from_slug(slug);
    model.display_name = "Claude Sonnet 4.6".to_string();
    model.description = Some("Anthropic Claude via native Messages API.".to_string());
    model.visibility = ModelVisibility::List;
    model.priority = 0;
    model.supports_parallel_tool_calls = true;
    model
}
