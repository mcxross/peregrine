use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use tokio::sync::TryLockError;

const PEREGRINE_IDENTITY: &str = "You are Peregrine, a coding agent. You and the user share one workspace, and your job is to collaborate with them until their goal is genuinely handled.";

pub(crate) fn wrap_models_manager(inner: SharedModelsManager) -> SharedModelsManager {
    std::sync::Arc::new(PeregrineModelsManager { inner })
}

#[derive(Debug)]
struct PeregrineModelsManager {
    inner: SharedModelsManager,
}

#[async_trait::async_trait]
impl ModelsManager for PeregrineModelsManager {
    async fn raw_model_catalog(&self, refresh_strategy: RefreshStrategy) -> ModelsResponse {
        normalize_model_catalog(self.inner.raw_model_catalog(refresh_strategy).await)
    }

    async fn get_remote_models(&self) -> Vec<ModelInfo> {
        self.inner
            .get_remote_models()
            .await
            .into_iter()
            .map(normalize_model_info)
            .collect()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.inner
            .try_get_remote_models()
            .map(|models| models.into_iter().map(normalize_model_info).collect())
    }

    fn auth_manager(&self) -> Option<&codex_login::AuthManager> {
        self.inner.auth_manager()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.inner.list_collaboration_modes()
    }

    async fn get_model_info(&self, model: &str, config: &ModelsManagerConfig) -> ModelInfo {
        normalize_model_info(self.inner.get_model_info(model, config).await)
    }

    async fn refresh_if_new_etag(&self, etag: String) {
        self.inner.refresh_if_new_etag(etag).await;
    }
}

fn normalize_model_catalog(mut catalog: ModelsResponse) -> ModelsResponse {
    catalog.models = catalog
        .models
        .into_iter()
        .map(normalize_model_info)
        .collect();
    catalog
}

pub fn normalize_model_info(mut model: ModelInfo) -> ModelInfo {
    model.base_instructions = normalize_model_instructions_text(&model.base_instructions);
    if let Some(messages) = model.model_messages.as_mut()
        && let Some(template) = messages.instructions_template.as_mut()
    {
        *template = normalize_model_instructions_text(template);
    }
    model
}

pub fn normalize_model_instructions_text(text: &str) -> String {
    let mut normalized = text.to_string();
    for upstream in upstream_identity_headers() {
        normalized = normalized.replace(&upstream, PEREGRINE_IDENTITY);
    }
    normalized = normalized.replace(&upstream_context_note(), "");
    normalized
}

pub fn normalize_persisted_base_instructions(text: String) -> String {
    normalize_model_instructions_text(&text)
}

fn upstream_product() -> &'static str {
    "Codex"
}

fn upstream_vendor() -> &'static str {
    "OpenAI"
}

fn upstream_cli_label() -> String {
    format!("{} CLI", upstream_product())
}

fn upstream_identity_headers() -> Vec<String> {
    let product = upstream_product();
    let vendor = upstream_vendor();
    let cli = upstream_cli_label();
    vec![
        format!(
            "You are a coding agent running in the {cli}, a terminal-based coding assistant. {cli} is an open source project led by {vendor}. You are expected to be precise, safe, and helpful."
        ),
        format!(
            "You are {product}, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals."
        ),
        format!(
            "You are {product}, based on GPT-5. You are running as a coding agent in the {cli} on a user's computer."
        ),
        format!(
            "You are GPT-5.2 running in the {cli}, a terminal-based coding assistant. {cli} is an open source project led by {vendor}. You are expected to be precise, safe, and helpful."
        ),
        format!(
            "You are GPT-5.1 running in the {cli}, a terminal-based coding assistant. {cli} is an open source project led by {vendor}. You are expected to be precise, safe, and helpful."
        ),
    ]
}

fn upstream_context_note() -> String {
    format!(
        "Within this context, {} refers to the open-source agentic coding interface (not the old {} language model built by {}).\n\n",
        upstream_product(),
        upstream_product(),
        upstream_vendor()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_upstream_fallback_identity() {
        let upstream_headers = upstream_identity_headers();
        let text = normalize_model_instructions_text(&format!(
            "{}\n\n{}# How you work",
            upstream_headers[0],
            upstream_context_note()
        ));

        assert!(text.starts_with(PEREGRINE_IDENTITY));
        assert!(!text.contains(&upstream_cli_label()));
        assert!(!text.contains("open-source project led by OpenAI"));
    }

    #[test]
    fn normalizes_personality_template_identity() {
        let upstream_headers = upstream_identity_headers();
        let text = normalize_model_instructions_text(&format!(
            "{}\n\n{{{{ personality }}}}\n\n# General",
            upstream_headers[1]
        ));

        assert!(text.starts_with(PEREGRINE_IDENTITY));
        assert!(text.contains("{{ personality }}"));
        assert!(!text.contains(&format!("You are {}", upstream_product())));
    }

    #[test]
    fn leaves_custom_instructions_unchanged() {
        let custom = "You are an internal security reviewer for this workspace.";

        assert_eq!(
            normalize_persisted_base_instructions(custom.to_string()),
            custom
        );
    }
}
