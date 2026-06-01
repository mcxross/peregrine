use std::sync::Arc;
use std::sync::Weak;

use codex_core::config::Config as CodexConfig;
use codex_core::config::ConfigBuilder as CodexConfigBuilder;
use codex_core::config::LoaderOverrides as CodexLoaderOverrides;
use codex_extension_api::AgentSpawnFuture;
use codex_extension_api::AgentSpawner;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadResumeInput;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ThreadStopInput;
use codex_features::Feature;
use codex_login::AuthManager;
use peregrine_app_server_protocol::ServerNotification;
use peregrine_app_server_protocol::ThreadGoalUpdatedNotification;
use peregrine_core::NewThread;
use peregrine_core::StartThreadOptions;
use peregrine_core::ThreadManager;
use peregrine_core::config::Config;
use peregrine_types::ThreadId;
use peregrine_types::config_types::WebSearchMode;
use peregrine_types::error::PeregrineErr;
use peregrine_types::protocol::Event;
use peregrine_types::protocol::EventMsg;
use toml::Value as TomlValue;

use crate::outgoing_message::OutgoingMessageSender;

pub(crate) fn thread_extensions<S>(
    guardian_agent_spawner: S,
    event_sink: Arc<dyn ExtensionEventSink>,
    auth_manager: Arc<AuthManager>,
) -> Arc<ExtensionRegistry<Config>>
where
    S: AgentSpawner<StartThreadOptions, Spawned = NewThread, Error = PeregrineErr>
        + Send
        + Sync
        + 'static,
{
    let mut builder = ExtensionRegistryBuilder::<Config>::with_event_sink(event_sink);
    install_codex_extensions(&mut builder, guardian_agent_spawner, auth_manager);
    Arc::new(builder.build())
}

fn install_codex_extensions<S>(
    builder: &mut ExtensionRegistryBuilder<Config>,
    guardian_agent_spawner: S,
    auth_manager: Arc<AuthManager>,
) where
    S: Send + Sync + 'static,
{
    let mut codex_builder =
        ExtensionRegistryBuilder::<CodexConfig>::with_event_sink(builder.event_sink());
    codex_guardian::install(&mut codex_builder, guardian_agent_spawner);
    codex_memories_extension::install(&mut codex_builder, codex_otel::global());
    codex_web_search_extension::install(&mut codex_builder, auth_manager.clone());
    codex_image_generation_extension::install(&mut codex_builder, auth_manager);

    let codex_registry = Arc::new(codex_builder.build());

    builder.thread_lifecycle_contributor(Arc::new(CodexThreadLifecycleBridge {
        registry: Arc::clone(&codex_registry),
    }));
    builder.config_contributor(Arc::new(CodexConfigContributorBridge {
        registry: Arc::clone(&codex_registry),
    }));

    for contributor in codex_registry.turn_lifecycle_contributors() {
        builder.turn_lifecycle_contributor(Arc::clone(contributor));
    }
    for contributor in codex_registry.token_usage_contributors() {
        builder.token_usage_contributor(Arc::clone(contributor));
    }
    for contributor in codex_registry.context_contributors() {
        builder.prompt_contributor(Arc::clone(contributor));
    }
    for contributor in codex_registry.tool_contributors() {
        builder.tool_contributor(Arc::clone(contributor));
    }
    for contributor in codex_registry.tool_lifecycle_contributors() {
        builder.tool_lifecycle_contributor(Arc::clone(contributor));
    }
    for contributor in codex_registry.turn_item_contributors() {
        builder.turn_item_contributor(Arc::clone(contributor));
    }
}

struct CodexThreadLifecycleBridge {
    registry: Arc<ExtensionRegistry<CodexConfig>>,
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<Config> for CodexThreadLifecycleBridge {
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        let codex_config = match upstream_extension_config(input.config).await {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(?err, "failed to build upstream extension config");
                return;
            }
        };
        input.thread_store.insert(codex_config.clone());

        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_start(ThreadStartInput {
                    config: &codex_config,
                    session_source: input.session_source,
                    persistent_thread_state_available: input.persistent_thread_state_available,
                    session_store: input.session_store,
                    thread_store: input.thread_store,
                })
                .await;
        }
    }

    async fn on_thread_resume(&self, input: ThreadResumeInput<'_>) {
        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_resume(ThreadResumeInput {
                    session_store: input.session_store,
                    thread_store: input.thread_store,
                })
                .await;
        }
    }

    async fn on_thread_idle(&self, input: ThreadIdleInput<'_>) {
        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_idle(ThreadIdleInput {
                    session_store: input.session_store,
                    thread_store: input.thread_store,
                })
                .await;
        }
    }

    async fn on_thread_stop(&self, input: ThreadStopInput<'_>) {
        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_stop(ThreadStopInput {
                    session_store: input.session_store,
                    thread_store: input.thread_store,
                })
                .await;
        }
    }
}

struct CodexConfigContributorBridge {
    registry: Arc<ExtensionRegistry<CodexConfig>>,
}

impl ConfigContributor<Config> for CodexConfigContributorBridge {
    fn on_config_changed(
        &self,
        session_store: &codex_extension_api::ExtensionData,
        thread_store: &codex_extension_api::ExtensionData,
        previous_config: &Config,
        new_config: &Config,
    ) {
        let previous_codex_config = thread_store
            .get::<CodexConfig>()
            .map(|config| (*config).clone())
            .or_else(|| upstream_extension_config_blocking(previous_config).ok());
        let Some(previous_codex_config) = previous_codex_config else {
            tracing::warn!("failed to build previous upstream extension config");
            return;
        };
        let new_codex_config = match upstream_extension_config_blocking(new_config) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(?err, "failed to build new upstream extension config");
                return;
            }
        };

        for contributor in self.registry.config_contributors() {
            contributor.on_config_changed(
                session_store,
                thread_store,
                &previous_codex_config,
                &new_codex_config,
            );
        }
        thread_store.insert(new_codex_config);
    }
}

async fn upstream_extension_config(config: &Config) -> std::io::Result<CodexConfig> {
    build_upstream_extension_config(
        config.peregrine_home.to_path_buf(),
        effective_config_overrides(config),
    )
    .await
}

async fn build_upstream_extension_config(
    peregrine_home: std::path::PathBuf,
    cli_overrides: Vec<(String, TomlValue)>,
) -> std::io::Result<CodexConfig> {
    let mut loader_overrides = CodexLoaderOverrides::without_managed_config_for_tests();
    loader_overrides.ignore_managed_requirements = true;
    loader_overrides.ignore_user_config = true;

    CodexConfigBuilder::default()
        .codex_home(peregrine_home.clone())
        .cli_overrides(cli_overrides)
        .loader_overrides(loader_overrides)
        .fallback_cwd(Some(peregrine_home))
        .build()
        .await
}

fn upstream_extension_config_blocking(config: &Config) -> std::io::Result<CodexConfig> {
    let peregrine_home = config.peregrine_home.to_path_buf();
    let cli_overrides = effective_config_overrides(config);
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(build_upstream_extension_config(
            peregrine_home,
            cli_overrides,
        ))
    })
    .join()
    .map_err(|_| std::io::Error::other("upstream extension config thread panicked"))?
}

fn effective_config_overrides(config: &Config) -> Vec<(String, TomlValue)> {
    let mut overrides: Vec<(String, TomlValue)> = match config.config_layer_stack.effective_config()
    {
        TomlValue::Table(table) => table.into_iter().collect(),
        _ => Vec::new(),
    };

    overrides.push((
        "model_provider".to_string(),
        TomlValue::String(config.model_provider_id.clone()),
    ));
    overrides.push((
        "web_search".to_string(),
        web_search_mode_value(config.web_search_mode.value()),
    ));
    overrides.push((
        "features.standalone_web_search".to_string(),
        TomlValue::Boolean(config.features.enabled(Feature::StandaloneWebSearch)),
    ));
    overrides.push((
        "features.imagegenext".to_string(),
        TomlValue::Boolean(config.features.enabled(Feature::ImageGenExt)),
    ));
    overrides.push((
        "features.memories".to_string(),
        TomlValue::Boolean(config.features.enabled(Feature::MemoryTool)),
    ));
    overrides.push((
        "memories.use_memories".to_string(),
        TomlValue::Boolean(config.memories.use_memories),
    ));
    overrides.push((
        "memories.dedicated_tools".to_string(),
        TomlValue::Boolean(config.memories.dedicated_tools),
    ));

    overrides
}

fn web_search_mode_value(mode: WebSearchMode) -> TomlValue {
    let mode = match mode {
        WebSearchMode::Disabled => "disabled",
        WebSearchMode::Cached => "cached",
        WebSearchMode::Live => "live",
    };
    TomlValue::String(mode.to_string())
}

pub(crate) fn app_server_extension_event_sink(
    outgoing: Arc<OutgoingMessageSender>,
) -> Arc<dyn ExtensionEventSink> {
    Arc::new(AppServerExtensionEventSink { outgoing })
}

struct AppServerExtensionEventSink {
    outgoing: Arc<OutgoingMessageSender>,
}

impl ExtensionEventSink for AppServerExtensionEventSink {
    fn emit(&self, event: Event) {
        match event.msg {
            EventMsg::ThreadGoalUpdated(thread_goal_event) => {
                self.outgoing
                    .try_send_server_notification(ServerNotification::ThreadGoalUpdated(
                        ThreadGoalUpdatedNotification {
                            thread_id: thread_goal_event.thread_id.to_string(),
                            turn_id: thread_goal_event.turn_id,
                            goal: thread_goal_event.goal.into(),
                        },
                    ));
            }
            msg => {
                tracing::debug!(event_id = %event.id, ?msg, "dropping unsupported extension event");
            }
        }
    }
}

pub(crate) fn guardian_agent_spawner(
    thread_manager: Weak<ThreadManager>,
) -> impl AgentSpawner<StartThreadOptions, Spawned = NewThread, Error = PeregrineErr> {
    move |forked_from_thread_id: ThreadId,
          options: StartThreadOptions|
          -> AgentSpawnFuture<'static, NewThread, PeregrineErr> {
        let thread_manager = thread_manager.clone();
        Box::pin(async move {
            let thread_manager = thread_manager.upgrade().ok_or_else(|| {
                PeregrineErr::UnsupportedOperation("thread manager dropped".to_string())
            })?;
            thread_manager
                .spawn_subagent(forked_from_thread_id, options)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use codex_analytics::AnalyticsEventsClient;
    use peregrine_app_server_protocol::ServerNotification;
    use peregrine_app_server_protocol::ThreadGoal as AppServerThreadGoal;
    use peregrine_app_server_protocol::ThreadGoalStatus as AppServerThreadGoalStatus;
    use peregrine_types::protocol::ThreadGoal;
    use peregrine_types::protocol::ThreadGoalStatus;
    use peregrine_types::protocol::ThreadGoalUpdatedEvent;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;

    #[tokio::test]
    async fn app_server_event_sink_forwards_thread_goal_updates() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(4);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            AnalyticsEventsClient::disabled(),
        ));
        let sink = app_server_extension_event_sink(outgoing);
        let thread_id = ThreadId::default();

        sink.emit(Event {
            id: "call-1".to_string(),
            msg: EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id,
                turn_id: Some("turn-1".to_string()),
                goal: ThreadGoal {
                    thread_id,
                    objective: "wire extension events".to_string(),
                    status: ThreadGoalStatus::Active,
                    token_budget: Some(123),
                    tokens_used: 45,
                    time_used_seconds: 6,
                    created_at: 7,
                    updated_at: 8,
                },
            }),
        });

        let envelope = timeout(Duration::from_secs(1), outgoing_rx.recv())
            .await
            .expect("timed out waiting for forwarded extension event")
            .expect("outgoing channel closed unexpectedly");
        let OutgoingEnvelope::Broadcast { message } = envelope else {
            panic!("expected broadcast notification");
        };
        let OutgoingMessage::AppServerNotification(ServerNotification::ThreadGoalUpdated(
            notification,
        )) = message
        else {
            panic!("expected thread goal updated notification");
        };

        assert_eq!(
            ThreadGoalUpdatedNotification {
                thread_id: thread_id.to_string(),
                turn_id: Some("turn-1".to_string()),
                goal: AppServerThreadGoal {
                    thread_id: thread_id.to_string(),
                    objective: "wire extension events".to_string(),
                    status: AppServerThreadGoalStatus::Active,
                    token_budget: Some(123),
                    tokens_used: 45,
                    time_used_seconds: 6,
                    created_at: 7,
                    updated_at: 8,
                },
            },
            notification
        );
    }
}
