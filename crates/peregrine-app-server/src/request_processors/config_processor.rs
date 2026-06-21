use std::sync::Arc;

use super::app_infos_to_api;
use super::connectors;
use crate::config_manager::ConfigManager;
use crate::config_manager_service::ConfigManagerError;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use codex_analytics::AnalyticsEventsClient;
use codex_features::canonical_feature_for_key;
use codex_features::feature_for_key;
use codex_login::AuthManager;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use peregrine_app_server_protocol::AgentRoleListParams;
use peregrine_app_server_protocol::AgentRoleListResponse;
use peregrine_app_server_protocol::AgentRoleReadParams;
use peregrine_app_server_protocol::AgentRoleReadResponse;
use peregrine_app_server_protocol::AgentRoleSource;
use peregrine_app_server_protocol::AgentRoleSummary;
use peregrine_app_server_protocol::AgentRoleWriteParams;
use peregrine_app_server_protocol::AgentRoleWriteResponse;
use peregrine_app_server_protocol::AppListUpdatedNotification;
use peregrine_app_server_protocol::ClientResponsePayload;
use peregrine_app_server_protocol::ComputerUseRequirements;
use peregrine_app_server_protocol::ConfigBatchWriteParams;
use peregrine_app_server_protocol::ConfigEdit;
use peregrine_app_server_protocol::ConfigReadParams;
use peregrine_app_server_protocol::ConfigReadResponse;
use peregrine_app_server_protocol::ConfigRequirements;
use peregrine_app_server_protocol::ConfigRequirementsReadResponse;
use peregrine_app_server_protocol::ConfigValueWriteParams;
use peregrine_app_server_protocol::ConfigWriteErrorCode;
use peregrine_app_server_protocol::ConfigWriteResponse;
use peregrine_app_server_protocol::ConfiguredHookHandler;
use peregrine_app_server_protocol::ConfiguredHookMatcherGroup;
use peregrine_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use peregrine_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use peregrine_app_server_protocol::JSONRPCErrorError;
use peregrine_app_server_protocol::ManagedHooksRequirements;
use peregrine_app_server_protocol::MergeStrategy;
use peregrine_app_server_protocol::ModelProviderCapabilitiesReadResponse;
use peregrine_app_server_protocol::ModelProviderCredentialState;
use peregrine_app_server_protocol::ModelProviderEntry;
use peregrine_app_server_protocol::ModelProviderListParams;
use peregrine_app_server_protocol::ModelProviderListResponse;
use peregrine_app_server_protocol::ModelProviderModel;
use peregrine_app_server_protocol::ModelProviderModelsListParams;
use peregrine_app_server_protocol::ModelProviderModelsListResponse;
use peregrine_app_server_protocol::ModelProviderSelectParams;
use peregrine_app_server_protocol::ModelProviderSelectResponse;
use peregrine_app_server_protocol::ModelProviderSelection;
use peregrine_app_server_protocol::NetworkDomainPermission;
use peregrine_app_server_protocol::NetworkRequirements;
use peregrine_app_server_protocol::NetworkUnixSocketPermission;
use peregrine_app_server_protocol::SandboxMode;
use peregrine_app_server_protocol::ServerNotification;
use peregrine_app_server_protocol::WindowsSandboxSetupMode;
use peregrine_config::ConfigRequirementsToml;
use peregrine_config::HookEventsToml;
use peregrine_config::HookHandlerConfig as CoreHookHandlerConfig;
use peregrine_config::ManagedHooksRequirementsToml;
use peregrine_config::MatcherGroup as CoreMatcherGroup;
use peregrine_config::ResidencyRequirement as CoreResidencyRequirement;
use peregrine_config::SandboxModeRequirement as CoreSandboxModeRequirement;
use peregrine_core::ThreadManager;
use peregrine_core::agent_role_catalog::AgentRoleCatalogEntry;
use peregrine_core::agent_role_catalog::AgentRoleCatalogSource as CoreAgentRoleCatalogSource;
use peregrine_model_provider::ANTHROPIC_API_KEY_ENV_VAR;
use peregrine_model_provider::ANTHROPIC_PROVIDER_ID;
use peregrine_model_provider::OLLAMA_DEFAULT_MODEL;
use peregrine_model_provider::ProviderAuthStrategy as CoreProviderAuthStrategy;
use peregrine_model_provider::ProviderCatalogEntry;
use peregrine_model_provider::ProviderKind as CoreProviderKind;
use peregrine_model_provider::ProviderWireApi as CoreProviderWireApi;
use peregrine_model_provider::provider_catalog_entries;
use peregrine_model_provider::provider_catalog_entry_for_id;
use peregrine_provider_registry::create_model_provider;
use peregrine_types::config_types::WebSearchMode;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

const OLLAMA_MODELS_LIST_TIMEOUT: Duration = Duration::from_secs(3);

const SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT: &[&str] = &[
    "apps",
    "memories",
    "mentions_v2",
    "plugins",
    "remote_control",
    "remote_plugin",
    "tool_suggest",
    "tool_call_mcp_elicitation",
];

fn agent_role_summary_from_catalog_entry(entry: AgentRoleCatalogEntry) -> AgentRoleSummary {
    AgentRoleSummary {
        name: entry.name,
        description: entry.description,
        source: match entry.source {
            CoreAgentRoleCatalogSource::BuiltIn => AgentRoleSource::BuiltIn,
            CoreAgentRoleCatalogSource::Configured => AgentRoleSource::Configured,
        },
        config_file: entry
            .config_file
            .map(|path| path.to_string_lossy().into_owned()),
        nickname_candidates: entry.nickname_candidates,
        overrides_built_in: entry.overrides_built_in,
    }
}

#[derive(Clone)]
pub(crate) struct ConfigRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    #[allow(dead_code)]
    analytics_events_client: AnalyticsEventsClient,
}

impl ConfigRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        config_manager: ConfigManager,
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        analytics_events_client: AnalyticsEventsClient,
    ) -> Self {
        Self {
            outgoing,
            config_manager,
            auth_manager,
            thread_manager,
            analytics_events_client,
        }
    }

    pub(crate) async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, JSONRPCErrorError> {
        let fallback_cwd = params.cwd.as_ref().map(PathBuf::from);
        let mut response = self.config_manager.read(params).await.map_err(map_error)?;
        let config = self.load_latest_config(fallback_cwd).await?;
        for feature_key in SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT {
            let Some(feature) = feature_for_key(feature_key) else {
                continue;
            };
            let features = response
                .config
                .additional
                .entry("features".to_string())
                .or_insert_with(|| json!({}));
            if !features.is_object() {
                *features = json!({});
            }
            if let Some(features) = features.as_object_mut() {
                features.insert(
                    (*feature_key).to_string(),
                    json!(config.features.enabled(feature)),
                );
            }
        }
        Ok(response)
    }

    pub(crate) async fn agent_role_list(
        &self,
        params: AgentRoleListParams,
    ) -> Result<AgentRoleListResponse, JSONRPCErrorError> {
        let fallback_cwd = params.cwd.map(PathBuf::from);
        let config = self.load_latest_config(fallback_cwd).await?;
        Ok(AgentRoleListResponse {
            roles: peregrine_core::agent_role_catalog::list_agent_roles(&config)
                .into_iter()
                .map(agent_role_summary_from_catalog_entry)
                .collect(),
        })
    }

    pub(crate) async fn agent_role_read(
        &self,
        params: AgentRoleReadParams,
    ) -> Result<AgentRoleReadResponse, JSONRPCErrorError> {
        self.config_manager
            .read_agent_role_edit(params)
            .await
            .map_err(map_error)
    }

    pub(crate) async fn agent_role_write(
        &self,
        params: AgentRoleWriteParams,
    ) -> Result<AgentRoleWriteResponse, JSONRPCErrorError> {
        let response = self
            .config_manager
            .write_agent_role_edit(params)
            .await
            .map_err(map_error)?;
        self.handle_config_mutation().await;
        self.reload_user_config().await;
        Ok(response)
    }

    pub(crate) async fn config_requirements_read(
        &self,
    ) -> Result<ConfigRequirementsReadResponse, JSONRPCErrorError> {
        let requirements = self
            .config_manager
            .read_requirements()
            .await
            .map_err(map_error)?
            .map(map_requirements_toml_to_api);

        Ok(ConfigRequirementsReadResponse { requirements })
    }

    pub(crate) async fn value_write(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ClientResponsePayload, JSONRPCErrorError> {
        self.handle_config_mutation_result(self.write_value(params).await)
            .await
            .map(ClientResponsePayload::ConfigValueWrite)
    }

    pub(crate) async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ClientResponsePayload, JSONRPCErrorError> {
        self.handle_config_mutation_result(self.batch_write_inner(params).await)
            .await
            .map(ClientResponsePayload::ConfigBatchWrite)
    }

    pub(crate) async fn experimental_feature_enablement_set(
        &self,
        request_id: ConnectionRequestId,
        params: ExperimentalFeatureEnablementSetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let should_refresh_apps_list = params.enablement.get("apps").copied() == Some(true);
        let response = self
            .handle_config_mutation_result(self.set_experimental_feature_enablement(params).await)
            .await?;
        self.outgoing
            .send_response_as(
                request_id,
                ClientResponsePayload::ExperimentalFeatureEnablementSet(response),
            )
            .await;
        if should_refresh_apps_list {
            self.refresh_apps_list_after_experimental_feature_enablement_set()
                .await;
        }
        Ok(None)
    }

    pub(crate) async fn model_provider_capabilities_read(
        &self,
    ) -> Result<ModelProviderCapabilitiesReadResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let provider = create_model_provider(config.model_provider, /*auth_manager*/ None);
        let capabilities = provider.capabilities();
        Ok(ModelProviderCapabilitiesReadResponse {
            namespace_tools: capabilities.namespace_tools,
            image_generation: capabilities.image_generation,
            web_search: capabilities.web_search,
        })
    }

    pub(crate) async fn model_provider_list(
        &self,
        _params: ModelProviderListParams,
    ) -> Result<ModelProviderListResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let credential_context = self.provider_credential_context().await;
        Ok(ModelProviderListResponse {
            selected_provider_id: config.model_provider_id.clone(),
            data: provider_catalog_entries(&config.model_provider_id, &config.model_providers)
                .into_iter()
                .map(|entry| {
                    model_provider_entry_from_catalog(
                        entry,
                        &config.model_providers,
                        &credential_context,
                    )
                })
                .collect(),
        })
    }

    pub(crate) async fn model_provider_select(
        &self,
        params: ModelProviderSelectParams,
    ) -> Result<ModelProviderSelectResponse, JSONRPCErrorError> {
        let response = self.model_provider_select_inner(params).await?;
        self.handle_config_mutation().await;
        Ok(response)
    }

    pub(crate) async fn model_provider_models_list(
        &self,
        params: ModelProviderModelsListParams,
    ) -> Result<ModelProviderModelsListResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let provider_id = params.provider_id;
        let provider = config
            .model_providers
            .get(&provider_id)
            .ok_or_else(|| invalid_request(format!("unknown model provider `{provider_id}`")))?;
        let entry = provider_catalog_entry_for_id(
            &config.model_provider_id,
            &config.model_providers,
            &provider_id,
        );

        let data = match entry.kind {
            CoreProviderKind::Ollama => list_ollama_installed_models(provider).await?,
            _ => entry
                .default_model
                .as_deref()
                .map(model_provider_model_from_name)
                .into_iter()
                .collect(),
        };

        Ok(ModelProviderModelsListResponse { provider_id, data })
    }

    pub(crate) async fn handle_config_mutation(&self) {
        self.thread_manager.plugins_manager().clear_cache();
        self.thread_manager.skills_manager().clear_cache();
    }

    async fn provider_credential_context(&self) -> ProviderCredentialContext {
        ProviderCredentialContext {
            openai_auth_present: self.auth_manager.auth().await.is_some(),
        }
    }

    async fn handle_config_mutation_result<T>(
        &self,
        result: std::result::Result<T, JSONRPCErrorError>,
    ) -> Result<T, JSONRPCErrorError> {
        let response = result?;
        self.handle_config_mutation().await;
        Ok(response)
    }

    async fn refresh_apps_list_after_experimental_feature_enablement_set(&self) {
        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    "failed to load config for apps list refresh after experimental feature enablement: {}",
                    error.message
                );
                return;
            }
        };
        let auth = self.auth_manager.auth().await;
        if !config.features.apps_enabled_for_auth(
            auth.as_ref()
                .is_some_and(codex_login::CodexAuth::uses_codex_backend),
        ) {
            return;
        }

        let outgoing = Arc::clone(&self.outgoing);
        let environment_manager = self.thread_manager.environment_manager();
        tokio::spawn(async move {
            let (all_connectors_result, accessible_connectors_result) = tokio::join!(
                connectors::list_all_connectors_with_options(&config, /*force_refetch*/ true),
                connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager(
                    &config,
                    /*force_refetch*/ true,
                    Arc::clone(&environment_manager),
                ),
            );
            let all_connectors = match all_connectors_result {
                Ok(connectors) => connectors,
                Err(err) => {
                    tracing::warn!(
                        "failed to force-refresh directory apps after experimental feature enablement: {err:#}"
                    );
                    return;
                }
            };
            let accessible_connectors = match accessible_connectors_result {
                Ok(status) => status.connectors,
                Err(err) => {
                    tracing::warn!(
                        "failed to force-refresh accessible apps after experimental feature enablement: {err:#}"
                    );
                    return;
                }
            };

            let data = app_infos_to_api(connectors::with_app_enabled_state(
                connectors::merge_connectors_with_accessible(
                    all_connectors,
                    accessible_connectors,
                    /*all_connectors_loaded*/ true,
                ),
                &config,
            ));
            outgoing
                .send_server_notification(ServerNotification::AppListUpdated(
                    AppListUpdatedNotification { data },
                ))
                .await;
        });
    }

    async fn load_latest_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> Result<peregrine_core::config::Config, JSONRPCErrorError> {
        self.config_manager
            .load_latest_config(fallback_cwd)
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to resolve feature override precedence: {err}"
                ))
            })
    }

    async fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let pending_changes = codex_core_plugins::toggles::collect_plugin_enabled_candidates(
            [(&params.key_path, &params.value)].into_iter(),
        );
        let response = self
            .config_manager
            .write_value(params)
            .await
            .map_err(map_error)?;
        self.emit_plugin_toggle_events(pending_changes).await;
        Ok(response)
    }

    async fn batch_write_inner(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let reload_user_config = params.reload_user_config;
        let pending_changes = codex_core_plugins::toggles::collect_plugin_enabled_candidates(
            params
                .edits
                .iter()
                .map(|edit| (&edit.key_path, &edit.value)),
        );
        let response = self
            .config_manager
            .batch_write(params)
            .await
            .map_err(map_error)?;
        self.emit_plugin_toggle_events(pending_changes).await;
        if reload_user_config {
            self.reload_user_config().await;
        }
        Ok(response)
    }

    async fn model_provider_select_inner(
        &self,
        params: ModelProviderSelectParams,
    ) -> Result<ModelProviderSelectResponse, JSONRPCErrorError> {
        let ModelProviderSelectParams { provider_id, model } = params;
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let entries = provider_catalog_entries(&config.model_provider_id, &config.model_providers);
        let entry = entries
            .into_iter()
            .find(|entry| entry.id == provider_id)
            .ok_or_else(|| invalid_request(format!("unknown model provider `{provider_id}`")))?;

        if !entry.selectable {
            let reason = entry
                .disabled_reason
                .clone()
                .unwrap_or_else(|| "provider is not selectable".to_string());
            return Err(invalid_request(format!(
                "model provider `{provider_id}` is not selectable: {reason}"
            )));
        }

        let selected_model = model.or_else(|| entry.default_model.clone());
        let mut edits = vec![replace_config_value(
            "model_provider",
            serde_json::json!(provider_id),
        )];
        if let Some(model) = selected_model.as_deref() {
            edits.push(replace_config_value("model", serde_json::json!(model)));
        }

        self.batch_write_inner(ConfigBatchWriteParams {
            edits,
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;

        let next_config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        self.refresh_runtime_model_provider(next_config.clone())
            .await;
        let selected_entry =
            provider_catalog_entries(&next_config.model_provider_id, &next_config.model_providers)
                .into_iter()
                .find(|entry| entry.id == provider_id)
                .ok_or_else(|| {
                    internal_error(format!(
                        "selected model provider `{provider_id}` disappeared after config write"
                    ))
                })?;

        let selected_provider =
            model_provider_selection_from_config(&next_config, &selected_entry).await;
        let credential_context = self.provider_credential_context().await;
        let provider = model_provider_entry_from_catalog(
            selected_entry,
            &next_config.model_providers,
            &credential_context,
        );

        Ok(ModelProviderSelectResponse {
            selected_provider,
            provider,
            model: next_config.model.clone(),
        })
    }

    async fn set_experimental_feature_enablement(
        &self,
        params: ExperimentalFeatureEnablementSetParams,
    ) -> Result<ExperimentalFeatureEnablementSetResponse, JSONRPCErrorError> {
        let ExperimentalFeatureEnablementSetParams { enablement } = params;
        for key in enablement.keys() {
            if canonical_feature_for_key(key).is_some() {
                if SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT.contains(&key.as_str()) {
                    continue;
                }

                return Err(invalid_request(format!(
                    "unsupported feature enablement `{key}`: currently supported features are {}",
                    SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT.join(", ")
                )));
            }

            let message = if let Some(feature) = feature_for_key(key) {
                format!(
                    "invalid feature enablement `{key}`: use canonical feature key `{}`",
                    feature.key()
                )
            } else {
                format!("invalid feature enablement `{key}`")
            };
            return Err(invalid_request(message));
        }

        if enablement.is_empty() {
            return Ok(ExperimentalFeatureEnablementSetResponse { enablement });
        }

        self.config_manager
            .extend_runtime_feature_enablement(
                enablement
                    .iter()
                    .map(|(name, enabled)| (name.clone(), *enabled)),
            )
            .map_err(|_| internal_error("failed to update feature enablement"))?;

        self.load_latest_config(/*fallback_cwd*/ None).await?;
        self.reload_user_config().await;

        Ok(ExperimentalFeatureEnablementSetResponse { enablement })
    }

    async fn reload_user_config(&self) {
        let next_config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    "failed to rebuild user config for runtime refresh: {}",
                    err.message
                );
                return;
            }
        };
        let thread_ids = self.thread_manager.list_thread_ids().await;
        for thread_id in thread_ids {
            let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
                continue;
            };
            thread.refresh_runtime_config(next_config.clone()).await;
        }
    }

    async fn refresh_runtime_model_provider(&self, next_config: peregrine_core::config::Config) {
        let thread_ids = self.thread_manager.list_thread_ids().await;
        for thread_id in thread_ids {
            let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
                continue;
            };
            thread
                .refresh_runtime_model_provider(next_config.clone())
                .await;
        }
    }

    async fn emit_plugin_toggle_events(
        &self,
        pending_changes: std::collections::BTreeMap<String, bool>,
    ) {
        let _ = pending_changes;
    }
}

fn replace_config_value(key_path: impl Into<String>, value: serde_json::Value) -> ConfigEdit {
    ConfigEdit {
        key_path: key_path.into(),
        value,
        merge_strategy: MergeStrategy::Replace,
    }
}

struct ProviderCredentialContext {
    openai_auth_present: bool,
}

fn model_provider_entry_from_catalog(
    entry: ProviderCatalogEntry,
    model_providers: &HashMap<String, codex_model_provider_info::ModelProviderInfo>,
    credential_context: &ProviderCredentialContext,
) -> ModelProviderEntry {
    let provider = model_providers.get(&entry.id);
    let (credential_state, setup_hint) =
        provider_credential_metadata(&entry, provider, credential_context);

    ModelProviderEntry {
        id: entry.id,
        display_name: entry.display_name,
        description: entry.description,
        kind: match entry.kind {
            CoreProviderKind::OpenAi => peregrine_app_server_protocol::ModelProviderKind::OpenAi,
            CoreProviderKind::Anthropic => {
                peregrine_app_server_protocol::ModelProviderKind::Anthropic
            }
            CoreProviderKind::Ollama => peregrine_app_server_protocol::ModelProviderKind::Ollama,
            CoreProviderKind::AmazonBedrock => {
                peregrine_app_server_protocol::ModelProviderKind::AmazonBedrock
            }
            CoreProviderKind::OpenAiCompatible => {
                peregrine_app_server_protocol::ModelProviderKind::OpenAiCompatible
            }
            CoreProviderKind::LocalOpenAiCompatible => {
                peregrine_app_server_protocol::ModelProviderKind::LocalOpenAiCompatible
            }
        },
        wire_api: match entry.wire_api {
            CoreProviderWireApi::Responses => {
                peregrine_app_server_protocol::ModelProviderWireApi::Responses
            }
            CoreProviderWireApi::ChatCompletions => {
                peregrine_app_server_protocol::ModelProviderWireApi::ChatCompletions
            }
            CoreProviderWireApi::AnthropicMessages => {
                peregrine_app_server_protocol::ModelProviderWireApi::AnthropicMessages
            }
        },
        auth_strategy: match entry.auth_strategy {
            CoreProviderAuthStrategy::AccountOrApiKey => {
                peregrine_app_server_protocol::ModelProviderAuthStrategy::AccountOrApiKey
            }
            CoreProviderAuthStrategy::ApiKey => {
                peregrine_app_server_protocol::ModelProviderAuthStrategy::ApiKey
            }
            CoreProviderAuthStrategy::Aws => {
                peregrine_app_server_protocol::ModelProviderAuthStrategy::Aws
            }
            CoreProviderAuthStrategy::None => {
                peregrine_app_server_protocol::ModelProviderAuthStrategy::None
            }
            CoreProviderAuthStrategy::External => {
                peregrine_app_server_protocol::ModelProviderAuthStrategy::External
            }
            CoreProviderAuthStrategy::Unsupported => {
                peregrine_app_server_protocol::ModelProviderAuthStrategy::Unsupported
            }
        },
        selected: entry.selected,
        configured: entry.configured,
        selectable: entry.selectable,
        disabled_reason: entry.disabled_reason,
        default_model: entry.default_model,
        credential_state: Some(credential_state),
        setup_hint,
    }
}

fn provider_credential_metadata(
    entry: &ProviderCatalogEntry,
    provider: Option<&codex_model_provider_info::ModelProviderInfo>,
    credential_context: &ProviderCredentialContext,
) -> (ModelProviderCredentialState, Option<String>) {
    match entry.auth_strategy {
        CoreProviderAuthStrategy::None => (ModelProviderCredentialState::NotRequired, None),
        CoreProviderAuthStrategy::AccountOrApiKey => {
            if credential_context.openai_auth_present || provider_has_static_credential(provider) {
                (ModelProviderCredentialState::Ready, None)
            } else {
                (
                    ModelProviderCredentialState::NeedsLogin,
                    Some("Use /login or set OPENAI_API_KEY before starting a turn.".to_string()),
                )
            }
        }
        CoreProviderAuthStrategy::ApiKey => {
            if provider_has_static_credential(provider) {
                (ModelProviderCredentialState::Ready, None)
            } else {
                (
                    ModelProviderCredentialState::MissingApiKey,
                    Some(api_key_setup_hint(entry, provider)),
                )
            }
        }
        CoreProviderAuthStrategy::Aws
        | CoreProviderAuthStrategy::External
        | CoreProviderAuthStrategy::Unsupported => (ModelProviderCredentialState::Unknown, None),
    }
}

fn provider_has_static_credential(
    provider: Option<&codex_model_provider_info::ModelProviderInfo>,
) -> bool {
    let Some(provider) = provider else {
        return false;
    };
    provider
        .experimental_bearer_token
        .as_deref()
        .is_some_and(|token| !token.trim().is_empty())
        || provider.env_key.as_deref().is_some_and(env_key_is_present)
        || provider.env_http_headers.as_ref().is_some_and(|headers| {
            headers
                .values()
                .any(|env_key| env_key_is_present(env_key.as_str()))
        })
}

fn api_key_setup_hint(
    entry: &ProviderCatalogEntry,
    provider: Option<&codex_model_provider_info::ModelProviderInfo>,
) -> String {
    if entry.id == ANTHROPIC_PROVIDER_ID {
        return format!("Set {ANTHROPIC_API_KEY_ENV_VAR} before starting a turn.");
    }
    if entry.id == OPENAI_PROVIDER_ID {
        return "Use /login or set OPENAI_API_KEY before starting a turn.".to_string();
    }
    if let Some(instructions) = provider
        .and_then(|provider| provider.env_key_instructions.as_deref())
        .filter(|instructions| !instructions.trim().is_empty())
    {
        return instructions.to_string();
    }
    if let Some(env_key) = provider
        .and_then(|provider| provider.env_key.as_deref())
        .filter(|env_key| !env_key.trim().is_empty())
    {
        return format!("Set {env_key} before starting a turn.");
    }

    "Configure an API key before starting a turn.".to_string()
}

fn env_key_is_present(env_key: &str) -> bool {
    !env_key.trim().is_empty()
        && std::env::var(env_key)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
}

async fn model_provider_selection_from_config(
    config: &peregrine_core::config::Config,
    entry: &ProviderCatalogEntry,
) -> ModelProviderSelection {
    let provider = create_model_provider(config.model_provider.clone(), /*auth_manager*/ None);
    let runtime_base_url = match provider.runtime_base_url().await {
        Ok(base_url) => base_url,
        Err(err) => {
            tracing::warn!(%err, "failed to resolve selected model provider runtime base URL");
            None
        }
    };

    ModelProviderSelection {
        id: config.model_provider_id.clone(),
        display_name: entry.display_name.clone(),
        requires_openai_auth: config.model_provider.requires_openai_auth,
        runtime_base_url,
    }
}

async fn list_ollama_installed_models(
    provider: &codex_model_provider_info::ModelProviderInfo,
) -> Result<Vec<ModelProviderModel>, JSONRPCErrorError> {
    let base_url = provider
        .base_url
        .as_deref()
        .ok_or_else(|| internal_error("Ollama provider is missing base_url"))?;
    let tags_url = format!("{}/api/tags", host_root_from_openai_base_url(base_url));
    let client = reqwest::Client::builder()
        .timeout(OLLAMA_MODELS_LIST_TIMEOUT)
        .build()
        .map_err(|err| internal_error(format!("failed to build Ollama HTTP client: {err}")))?;
    let response = client.get(tags_url).send().await.map_err(|err| {
        invalid_request(format!(
            "No running Ollama server detected. Start it with `ollama serve`: {err}"
        ))
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(invalid_request(format!(
            "Ollama model list request failed with HTTP {status}"
        )));
    }

    let value = response.json::<serde_json::Value>().await.map_err(|err| {
        invalid_request(format!("failed to read Ollama model list response: {err}"))
    })?;
    let mut models = value
        .get("models")
        .and_then(|models| models.as_array())
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model.get("name").and_then(|name| name.as_str()))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    models.sort();
    models.dedup();

    Ok(models
        .into_iter()
        .map(|model| {
            let is_default = model == OLLAMA_DEFAULT_MODEL;
            model_provider_model(
                model,
                is_default,
                Some("Installed Ollama model".to_string()),
            )
        })
        .collect())
}

fn model_provider_model_from_name(model: &str) -> ModelProviderModel {
    model_provider_model(model.to_string(), true, None)
}

fn model_provider_model(
    model: String,
    is_default: bool,
    description: Option<String>,
) -> ModelProviderModel {
    ModelProviderModel {
        id: model.clone(),
        display_name: model.clone(),
        model,
        description,
        is_default,
    }
}

fn host_root_from_openai_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    trimmed
        .strip_suffix("/v1")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .to_string()
}

fn map_requirements_toml_to_api(requirements: ConfigRequirementsToml) -> ConfigRequirements {
    ConfigRequirements {
        allowed_approval_policies: requirements.allowed_approval_policies.map(|policies| {
            policies
                .into_iter()
                .map(peregrine_app_server_protocol::AskForApproval::from)
                .collect()
        }),
        allowed_approvals_reviewers: requirements.allowed_approvals_reviewers.map(|reviewers| {
            reviewers
                .into_iter()
                .map(peregrine_app_server_protocol::ApprovalsReviewer::from)
                .collect()
        }),
        allowed_sandbox_modes: requirements.allowed_sandbox_modes.map(|modes| {
            modes
                .into_iter()
                .filter_map(map_sandbox_mode_requirement_to_api)
                .collect()
        }),
        allowed_windows_sandbox_implementations: requirements.windows.and_then(|windows| {
            windows
                .allowed_sandbox_implementations
                .map(|implementations| {
                    implementations
                        .into_iter()
                        .map(|implementation| match implementation {
                            peregrine_config::types::WindowsSandboxModeToml::Elevated => {
                                WindowsSandboxSetupMode::Elevated
                            }
                            peregrine_config::types::WindowsSandboxModeToml::Unelevated => {
                                WindowsSandboxSetupMode::Unelevated
                            }
                        })
                        .collect()
                })
        }),
        allowed_permissions: requirements.allowed_permissions,
        allowed_web_search_modes: requirements.allowed_web_search_modes.map(|modes| {
            let mut normalized = modes
                .into_iter()
                .map(Into::into)
                .collect::<Vec<WebSearchMode>>();
            if !normalized.contains(&WebSearchMode::Disabled) {
                normalized.push(WebSearchMode::Disabled);
            }
            normalized
        }),
        allow_managed_hooks_only: requirements.allow_managed_hooks_only,
        allow_appshots: requirements.allow_appshots,
        computer_use: requirements
            .computer_use
            .map(map_computer_use_requirements_to_api),
        feature_requirements: requirements
            .feature_requirements
            .map(|requirements| requirements.entries),
        hooks: requirements.hooks.map(map_hooks_requirements_to_api),
        enforce_residency: requirements
            .enforce_residency
            .map(map_residency_requirement_to_api),
        network: requirements.network.map(map_network_requirements_to_api),
    }
}

fn map_computer_use_requirements_to_api(
    computer_use: peregrine_config::ComputerUseRequirementsToml,
) -> ComputerUseRequirements {
    ComputerUseRequirements {
        allow_locked_computer_use: computer_use.allow_locked_computer_use,
    }
}

fn map_hooks_requirements_to_api(hooks: ManagedHooksRequirementsToml) -> ManagedHooksRequirements {
    let ManagedHooksRequirementsToml {
        managed_dir,
        windows_managed_dir,
        hooks,
    } = hooks;
    let HookEventsToml {
        pre_tool_use,
        permission_request,
        post_tool_use,
        pre_compact,
        post_compact,
        session_start,
        user_prompt_submit,
        subagent_start,
        subagent_stop,
        stop,
    } = hooks;

    ManagedHooksRequirements {
        managed_dir,
        windows_managed_dir,
        pre_tool_use: map_hook_matcher_groups_to_api(pre_tool_use),
        permission_request: map_hook_matcher_groups_to_api(permission_request),
        post_tool_use: map_hook_matcher_groups_to_api(post_tool_use),
        pre_compact: map_hook_matcher_groups_to_api(pre_compact),
        post_compact: map_hook_matcher_groups_to_api(post_compact),
        session_start: map_hook_matcher_groups_to_api(session_start),
        user_prompt_submit: map_hook_matcher_groups_to_api(user_prompt_submit),
        subagent_start: map_hook_matcher_groups_to_api(subagent_start),
        subagent_stop: map_hook_matcher_groups_to_api(subagent_stop),
        stop: map_hook_matcher_groups_to_api(stop),
    }
}

fn map_hook_matcher_groups_to_api(
    groups: Vec<CoreMatcherGroup>,
) -> Vec<ConfiguredHookMatcherGroup> {
    groups
        .into_iter()
        .map(map_hook_matcher_group_to_api)
        .collect()
}

fn map_hook_matcher_group_to_api(group: CoreMatcherGroup) -> ConfiguredHookMatcherGroup {
    ConfiguredHookMatcherGroup {
        matcher: group.matcher,
        hooks: group
            .hooks
            .into_iter()
            .map(map_hook_handler_to_api)
            .collect(),
    }
}

fn map_hook_handler_to_api(handler: CoreHookHandlerConfig) -> ConfiguredHookHandler {
    match handler {
        CoreHookHandlerConfig::Command {
            command,
            command_windows,
            timeout_sec,
            r#async,
            status_message,
        } => ConfiguredHookHandler::Command {
            command,
            command_windows,
            timeout_sec,
            r#async,
            status_message,
        },
        CoreHookHandlerConfig::Prompt {} => ConfiguredHookHandler::Prompt {},
        CoreHookHandlerConfig::Agent {} => ConfiguredHookHandler::Agent {},
    }
}

fn map_sandbox_mode_requirement_to_api(mode: CoreSandboxModeRequirement) -> Option<SandboxMode> {
    match mode {
        CoreSandboxModeRequirement::ReadOnly => Some(SandboxMode::ReadOnly),
        CoreSandboxModeRequirement::WorkspaceWrite => Some(SandboxMode::WorkspaceWrite),
        CoreSandboxModeRequirement::DangerFullAccess => Some(SandboxMode::DangerFullAccess),
        CoreSandboxModeRequirement::ExternalSandbox => None,
    }
}

fn map_residency_requirement_to_api(
    residency: CoreResidencyRequirement,
) -> peregrine_app_server_protocol::ResidencyRequirement {
    match residency {
        CoreResidencyRequirement::Us => peregrine_app_server_protocol::ResidencyRequirement::Us,
    }
}

fn map_network_requirements_to_api(
    network: peregrine_config::NetworkRequirementsToml,
) -> NetworkRequirements {
    let allowed_domains = network
        .domains
        .as_ref()
        .and_then(peregrine_config::NetworkDomainPermissionsToml::allowed_domains);
    let denied_domains = network
        .domains
        .as_ref()
        .and_then(peregrine_config::NetworkDomainPermissionsToml::denied_domains);
    let allow_unix_sockets = network
        .unix_sockets
        .as_ref()
        .map(peregrine_config::NetworkUnixSocketPermissionsToml::allow_unix_sockets)
        .filter(|entries| !entries.is_empty());

    NetworkRequirements {
        enabled: network.enabled,
        http_port: network.http_port,
        socks_port: network.socks_port,
        allow_upstream_proxy: network.allow_upstream_proxy,
        dangerously_allow_non_loopback_proxy: network.dangerously_allow_non_loopback_proxy,
        dangerously_allow_all_unix_sockets: network.dangerously_allow_all_unix_sockets,
        domains: network.domains.map(|domains| {
            domains
                .entries
                .into_iter()
                .map(|(pattern, permission)| {
                    (pattern, map_network_domain_permission_to_api(permission))
                })
                .collect()
        }),
        managed_allowed_domains_only: network.managed_allowed_domains_only,
        allowed_domains,
        denied_domains,
        unix_sockets: network.unix_sockets.map(|unix_sockets| {
            unix_sockets
                .entries
                .into_iter()
                .map(|(path, permission)| {
                    (path, map_network_unix_socket_permission_to_api(permission))
                })
                .collect()
        }),
        allow_unix_sockets,
        allow_local_binding: network.allow_local_binding,
    }
}

fn map_network_domain_permission_to_api(
    permission: peregrine_config::NetworkDomainPermissionToml,
) -> NetworkDomainPermission {
    match permission {
        peregrine_config::NetworkDomainPermissionToml::Allow => NetworkDomainPermission::Allow,
        peregrine_config::NetworkDomainPermissionToml::Deny => NetworkDomainPermission::Deny,
    }
}

fn map_network_unix_socket_permission_to_api(
    permission: peregrine_config::NetworkUnixSocketPermissionToml,
) -> NetworkUnixSocketPermission {
    match permission {
        peregrine_config::NetworkUnixSocketPermissionToml::Allow => {
            NetworkUnixSocketPermission::Allow
        }
        peregrine_config::NetworkUnixSocketPermissionToml::Deny => {
            NetworkUnixSocketPermission::Deny
        }
    }
}

fn map_error(err: ConfigManagerError) -> JSONRPCErrorError {
    if let Some(code) = err.write_error_code() {
        return config_write_error(code, err.to_string());
    }

    internal_error(err.to_string())
}

fn config_write_error(code: ConfigWriteErrorCode, message: impl Into<String>) -> JSONRPCErrorError {
    let mut error = invalid_request(message);
    error.data = Some(json!({
        "config_write_error_code": code,
    }));
    error
}

#[cfg(test)]
mod tests {
    use super::host_root_from_openai_base_url;
    use super::map_requirements_toml_to_api;
    use peregrine_app_server_protocol::WindowsSandboxSetupMode;
    use peregrine_config::ComputerUseRequirementsToml;
    use peregrine_config::ConfigRequirementsToml;
    use peregrine_config::WindowsRequirementsToml;
    use pretty_assertions::assert_eq;

    #[test]
    fn host_root_from_openai_base_url_strips_v1_suffix() {
        assert_eq!(
            host_root_from_openai_base_url("http://localhost:11434/v1"),
            "http://localhost:11434"
        );
        assert_eq!(
            host_root_from_openai_base_url("http://localhost:11434/v1/"),
            "http://localhost:11434"
        );
        assert_eq!(
            host_root_from_openai_base_url("http://localhost:11434"),
            "http://localhost:11434"
        );
    }

    #[test]
    fn requirements_api_includes_allow_managed_hooks_only() {
        let mapped = map_requirements_toml_to_api(ConfigRequirementsToml {
            allowed_permissions: Some(vec![
                "managed-standard".to_string(),
                "managed-build".to_string(),
            ]),
            allow_managed_hooks_only: Some(true),
            ..ConfigRequirementsToml::default()
        });

        assert_eq!(
            mapped.allowed_permissions,
            Some(vec![
                "managed-standard".to_string(),
                "managed-build".to_string(),
            ])
        );
        assert_eq!(mapped.allow_managed_hooks_only, Some(true));
        assert_eq!(mapped.hooks, None);
    }

    #[test]
    fn requirements_api_includes_allow_appshots() {
        let mapped = map_requirements_toml_to_api(ConfigRequirementsToml {
            allow_appshots: Some(false),
            ..ConfigRequirementsToml::default()
        });

        assert_eq!(mapped.allow_appshots, Some(false));
        assert_eq!(mapped.hooks, None);
    }

    #[test]
    fn requirements_api_includes_computer_use_requirements() {
        let mapped = map_requirements_toml_to_api(ConfigRequirementsToml {
            computer_use: Some(ComputerUseRequirementsToml {
                allow_locked_computer_use: Some(false),
            }),
            ..ConfigRequirementsToml::default()
        });

        assert_eq!(
            mapped
                .computer_use
                .and_then(|requirements| requirements.allow_locked_computer_use),
            Some(false)
        );
    }

    #[test]
    fn requirements_api_includes_allowed_windows_sandbox_implementations() {
        let mapped = map_requirements_toml_to_api(ConfigRequirementsToml {
            windows: Some(WindowsRequirementsToml {
                allowed_sandbox_implementations: Some(vec![
                    peregrine_config::types::WindowsSandboxModeToml::Elevated,
                    peregrine_config::types::WindowsSandboxModeToml::Unelevated,
                ]),
            }),
            ..ConfigRequirementsToml::default()
        });

        assert_eq!(
            mapped.allowed_windows_sandbox_implementations,
            Some(vec![
                WindowsSandboxSetupMode::Elevated,
                WindowsSandboxSetupMode::Unelevated,
            ])
        );
    }
}
