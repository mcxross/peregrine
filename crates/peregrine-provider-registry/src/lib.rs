//! Built-in model provider registry.
//!
//! Concrete provider implementations live in provider-specific crates. This
//! crate is the narrow assembly point used by app-server/core runtime code.

use std::sync::Arc;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use peregrine_anthropic::AnthropicModelProvider;
use peregrine_model_provider::SharedModelProvider;
use peregrine_model_provider::is_anthropic_provider_info;

pub fn create_model_provider(
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
) -> SharedModelProvider {
    if is_anthropic_provider_info(&provider_info) {
        Arc::new(AnthropicModelProvider::new(provider_info))
    } else {
        peregrine_model_provider::create_model_provider(provider_info, auth_manager)
    }
}
