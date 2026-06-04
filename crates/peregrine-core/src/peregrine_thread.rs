use crate::agent::AgentStatus;
use crate::config::ConstraintResult;
use crate::goals::ExternalGoalSet;
use crate::goals::GoalRuntimeEvent;
use crate::session::Peregrine;
use crate::session::SessionSettingsUpdate;
use crate::session::SteerInputError;
use codex_features::Feature;
use codex_otel::SessionTelemetry;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::ThreadStoreResult;
use codex_utils_absolute_path::AbsolutePathBuf;
use peregrine_types::config_types::ApprovalsReviewer;
use peregrine_types::config_types::CollaborationMode;
use peregrine_types::config_types::Personality;
use peregrine_types::config_types::ReasoningSummary;
use peregrine_types::config_types::WindowsSandboxLevel;
use peregrine_types::error::PeregrineErr;
use peregrine_types::error::Result as PeregrineResult;
use peregrine_types::mcp::CallToolResult;
use peregrine_types::models::ActivePermissionProfile;
use peregrine_types::models::ContentItem;
use peregrine_types::models::PermissionProfile;
use peregrine_types::models::ResponseItem;
use peregrine_types::openai_models::ReasoningEffort;
use peregrine_types::protocol::AdditionalContextEntry;
use peregrine_types::protocol::AskForApproval;
use peregrine_types::protocol::Event;
use peregrine_types::protocol::Op;
use peregrine_types::protocol::SandboxPolicy;
use peregrine_types::protocol::SessionConfiguredEvent;
use peregrine_types::protocol::SessionSource;
use peregrine_types::protocol::Submission;
use peregrine_types::protocol::ThreadMemoryMode;
use peregrine_types::protocol::ThreadSource;
use peregrine_types::protocol::TokenUsageInfo;
use peregrine_types::protocol::TurnEnvironmentSelection;
use peregrine_types::protocol::W3cTraceContext;
use peregrine_types::user_input::UserInput;
use rmcp::model::ReadResourceRequestParams;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;

use codex_rollout::state_db::StateDbHandle;

#[derive(Clone, Debug)]
pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub service_tier: Option<String>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub permission_profile: PermissionProfile,
    pub active_permission_profile: Option<ActivePermissionProfile>,
    pub cwd: AbsolutePathBuf,
    pub workspace_roots: Vec<AbsolutePathBuf>,
    pub profile_workspace_roots: Vec<AbsolutePathBuf>,
    pub ephemeral: bool,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub personality: Option<Personality>,
    pub collaboration_mode: CollaborationMode,
    pub session_source: SessionSource,
    pub thread_source: Option<ThreadSource>,
}

impl ThreadConfigSnapshot {
    pub fn sandbox_policy(&self) -> SandboxPolicy {
        let file_system_sandbox_policy = self.permission_profile.file_system_sandbox_policy();
        codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
            &self.permission_profile,
            &file_system_sandbox_policy,
            self.permission_profile.network_sandbox_policy(),
            self.cwd.as_path(),
        )
    }
}

/// Thread settings overrides that app-server validates before starting a turn.
#[derive(Clone, Default)]
pub struct PeregrineThreadSettingsOverrides {
    pub cwd: Option<PathBuf>,
    pub workspace_roots: Option<Vec<AbsolutePathBuf>>,
    pub profile_workspace_roots: Option<Vec<AbsolutePathBuf>>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub permission_profile: Option<PermissionProfile>,
    pub active_permission_profile: Option<ActivePermissionProfile>,
    pub windows_sandbox_level: Option<WindowsSandboxLevel>,
    pub model: Option<String>,
    pub effort: Option<Option<ReasoningEffort>>,
    pub summary: Option<ReasoningSummary>,
    pub service_tier: Option<Option<String>>,
    pub collaboration_mode: Option<CollaborationMode>,
    pub personality: Option<Personality>,
}

pub struct PeregrineThread {
    pub(crate) peregrine: Peregrine,
    pub(crate) session_source: SessionSource,
    session_configured: SessionConfiguredEvent,
    rollout_path: Option<PathBuf>,
    out_of_band_elicitation_count: Mutex<u64>,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Peregrine.
impl PeregrineThread {
    pub(crate) fn new(
        peregrine: Peregrine,
        session_configured: SessionConfiguredEvent,
        rollout_path: Option<PathBuf>,
        session_source: SessionSource,
    ) -> Self {
        Self {
            peregrine,
            session_source,
            session_configured,
            rollout_path,
            out_of_band_elicitation_count: Mutex::new(0),
        }
    }

    pub async fn submit(&self, op: Op) -> PeregrineResult<String> {
        self.peregrine.submit(op).await
    }

    /// Returns the session telemetry handle for thread-scoped production instrumentation.
    pub fn session_telemetry(&self) -> SessionTelemetry {
        self.peregrine.session.services.session_telemetry.clone()
    }

    pub async fn shutdown_and_wait(&self) -> PeregrineResult<()> {
        self.peregrine.shutdown_and_wait().await
    }

    /// Wait until the underlying session loop has terminated.
    pub async fn wait_until_terminated(&self) {
        self.peregrine.session_loop_termination.clone().await;
    }

    pub(crate) async fn emit_thread_resume_lifecycle(&self) {
        for contributor in self
            .peregrine
            .session
            .services
            .extensions
            .thread_lifecycle_contributors()
        {
            contributor
                .on_thread_resume(codex_extension_api::ThreadResumeInput {
                    session_store: &self.peregrine.session.services.session_extension_data,
                    thread_store: &self.peregrine.session.services.thread_extension_data,
                })
                .await;
        }
    }

    pub async fn apply_goal_resume_runtime_effects(&self) -> anyhow::Result<()> {
        self.peregrine
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ThreadResumed)
            .await
    }

    pub async fn continue_active_goal_if_idle(&self) -> anyhow::Result<()> {
        self.peregrine
            .session
            .goal_runtime_apply(GoalRuntimeEvent::MaybeContinueIfIdle)
            .await
    }

    pub async fn prepare_external_goal_mutation(&self) {
        if let Err(err) = self
            .peregrine
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalMutationStarting)
            .await
        {
            tracing::warn!("failed to prepare external goal mutation: {err}");
        }
    }

    pub async fn apply_external_goal_set(&self, external_set: ExternalGoalSet) {
        if let Err(err) = self
            .peregrine
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalSet { external_set })
            .await
        {
            tracing::warn!("failed to apply external goal status runtime effects: {err}");
        }
    }

    pub async fn apply_external_goal_clear(&self) {
        if let Err(err) = self
            .peregrine
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalClear)
            .await
        {
            tracing::warn!("failed to apply external goal clear runtime effects: {err}");
        }
    }

    #[doc(hidden)]
    pub async fn ensure_rollout_materialized(&self) {
        self.peregrine.session.ensure_rollout_materialized().await;
    }

    #[doc(hidden)]
    pub async fn flush_rollout(&self) -> std::io::Result<()> {
        self.peregrine.session.flush_rollout().await
    }

    pub async fn submit_with_trace(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
    ) -> PeregrineResult<String> {
        self.peregrine.submit_with_trace(op, trace).await
    }

    pub async fn submit_user_input_with_client_user_message_id(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
        client_user_message_id: Option<String>,
    ) -> PeregrineResult<String> {
        self.peregrine
            .submit_user_input_with_client_user_message_id(op, trace, client_user_message_id)
            .await
    }

    /// Persist whether this thread is eligible for future memory generation.
    pub async fn set_thread_memory_mode(&self, mode: ThreadMemoryMode) -> anyhow::Result<()> {
        self.peregrine.set_thread_memory_mode(mode).await
    }

    pub async fn steer_input(
        &self,
        input: Vec<UserInput>,
        additional_context: BTreeMap<String, AdditionalContextEntry>,
        expected_turn_id: Option<&str>,
        client_user_message_id: Option<String>,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, SteerInputError> {
        self.peregrine
            .steer_input(
                input,
                additional_context,
                expected_turn_id,
                client_user_message_id,
                responsesapi_client_metadata,
            )
            .await
    }

    /// Injects model-visible items into the currently active turn.
    ///
    /// This is the thread-level bridge to `Session::inject_if_running` for
    /// callers that only hold a `PeregrineThread`.
    /// It returns the unchanged items when this thread has no active turn.
    pub async fn inject_if_running(
        &self,
        items: Vec<ResponseItem>,
    ) -> Result<(), Vec<ResponseItem>> {
        self.peregrine.session.inject_if_running(items).await
    }

    pub async fn set_app_server_client_info(
        &self,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
        mcp_elicitations_auto_deny: bool,
    ) -> ConstraintResult<()> {
        self.peregrine
            .set_app_server_client_info(
                app_server_client_name,
                app_server_client_version,
                mcp_elicitations_auto_deny,
            )
            .await
    }

    /// Preview persistent thread settings overrides without committing them.
    pub async fn preview_thread_settings_overrides(
        &self,
        overrides: PeregrineThreadSettingsOverrides,
    ) -> ConstraintResult<ThreadConfigSnapshot> {
        let updates = self.thread_settings_update(overrides).await;
        self.peregrine.session.preview_settings(&updates).await
    }

    async fn thread_settings_update(
        &self,
        overrides: PeregrineThreadSettingsOverrides,
    ) -> SessionSettingsUpdate {
        let PeregrineThreadSettingsOverrides {
            cwd,
            workspace_roots,
            profile_workspace_roots,
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
            permission_profile,
            active_permission_profile,
            windows_sandbox_level,
            model,
            effort,
            summary,
            service_tier,
            collaboration_mode,
            personality,
        } = overrides;
        let collaboration_mode = if let Some(collaboration_mode) = collaboration_mode {
            collaboration_mode
        } else {
            self.peregrine
                .session
                .collaboration_mode()
                .await
                .with_updates(model, effort, /*developer_instructions*/ None)
        };

        SessionSettingsUpdate {
            cwd,
            workspace_roots,
            profile_workspace_roots,
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
            permission_profile,
            active_permission_profile,
            windows_sandbox_level,
            collaboration_mode: Some(collaboration_mode),
            reasoning_summary: summary,
            service_tier,
            personality,
            ..Default::default()
        }
    }

    /// Use sparingly: this is intended to be removed soon.
    pub async fn submit_with_id(&self, sub: Submission) -> PeregrineResult<()> {
        self.peregrine.submit_with_id(sub).await
    }

    pub async fn next_event(&self) -> PeregrineResult<Event> {
        self.peregrine.next_event().await
    }

    pub async fn agent_status(&self) -> AgentStatus {
        self.peregrine.agent_status().await
    }

    pub(crate) fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.peregrine.agent_status.clone()
    }

    /// Returns the complete token usage snapshot currently cached for this thread.
    ///
    /// This accessor is intentionally narrower than direct session access: it lets
    /// app-server lifecycle paths replay restored usage after resume or fork without
    /// exposing broader session mutation authority. A caller that only reads
    /// `total_token_usage` would drop last-turn usage and make the v2
    /// `thread/tokenUsage/updated` payload incomplete.
    pub async fn token_usage_info(&self) -> Option<TokenUsageInfo> {
        self.peregrine.session.token_usage_info().await
    }

    /// Records a user-role session-prefix message without creating a new user turn boundary.
    pub(crate) async fn inject_user_message_without_turn(&self, message: String) {
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: message }],
            phase: None,
        };
        self.peregrine
            .session
            .inject_no_new_turn(vec![item], /*current_turn_context*/ None)
            .await;
    }

    /// Record raw Responses API items without starting a new turn.
    pub async fn inject_response_items(&self, items: Vec<ResponseItem>) -> PeregrineResult<()> {
        if items.is_empty() {
            return Err(PeregrineErr::InvalidRequest(
                "items must not be empty".to_string(),
            ));
        }

        let turn_context = self.peregrine.session.new_default_turn().await;
        if self
            .peregrine
            .session
            .reference_context_item()
            .await
            .is_none()
        {
            self.peregrine
                .session
                .record_context_updates_and_set_reference_context_item(turn_context.as_ref())
                .await;
        }
        self.peregrine
            .session
            .inject_no_new_turn(items, Some(turn_context.as_ref()))
            .await;
        self.peregrine.session.flush_rollout().await?;
        Ok(())
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        self.rollout_path.clone()
    }

    pub fn session_configured(&self) -> SessionConfiguredEvent {
        self.session_configured.clone()
    }

    pub(crate) fn is_running(&self) -> bool {
        !self.peregrine.tx_sub.is_closed()
    }

    pub async fn guardian_trunk_rollout_path(&self) -> Option<PathBuf> {
        self.peregrine
            .session
            .guardian_review_session
            .trunk_rollout_path()
            .await
    }

    pub async fn load_history(
        &self,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let live_thread = self
            .peregrine
            .session
            .live_thread_for_persistence("load history")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread.load_history(include_archived).await
    }

    pub async fn read_thread(
        &self,
        include_archived: bool,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        let live_thread = self
            .peregrine
            .session
            .live_thread_for_persistence("read thread")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread
            .read_thread(include_archived, include_history)
            .await
    }

    pub async fn update_thread_metadata(
        &self,
        patch: ThreadMetadataPatch,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThread> {
        let live_thread = self
            .peregrine
            .session
            .live_thread_for_persistence("update thread metadata")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread.update_metadata(patch, include_archived).await
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.peregrine.state_db()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.peregrine.thread_config_snapshot().await
    }

    pub async fn config(&self) -> Arc<crate::config::Config> {
        self.peregrine.session.get_config().await
    }

    /// Refresh the thread's layer-backed user config state from a caller-supplied
    /// config snapshot. Thread-scoped layers and session-static settings remain
    /// unchanged.
    pub async fn refresh_runtime_config(&self, next_config: crate::config::Config) {
        self.peregrine
            .session
            .refresh_runtime_config(next_config)
            .await;
    }

    /// Refresh the selected model provider/model for already-open sessions after
    /// an explicit provider selection.
    pub async fn refresh_runtime_model_provider(&self, next_config: crate::config::Config) {
        self.peregrine
            .session
            .refresh_runtime_model_provider(next_config)
            .await;
    }

    pub async fn environment_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.peregrine.thread_environment_selections().await
    }

    pub async fn read_mcp_resource(
        &self,
        server: &str,
        uri: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let result = self
            .peregrine
            .session
            .read_resource(server, ReadResourceRequestParams::new(uri))
            .await?;

        Ok(serde_json::to_value(result)?)
    }

    pub async fn call_mcp_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.peregrine
            .session
            .call_tool(server, tool, arguments, meta)
            .await
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.peregrine.enabled(feature)
    }

    pub async fn increment_out_of_band_elicitation_count(&self) -> PeregrineResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        let was_zero = *guard == 0;
        *guard = guard.checked_add(1).ok_or_else(|| {
            PeregrineErr::Fatal("out-of-band elicitation count overflowed".to_string())
        })?;

        if was_zero {
            self.peregrine
                .session
                .set_out_of_band_elicitation_pause_state(/*paused*/ true);
        }

        Ok(*guard)
    }

    pub async fn decrement_out_of_band_elicitation_count(&self) -> PeregrineResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        if *guard == 0 {
            return Err(PeregrineErr::InvalidRequest(
                "out-of-band elicitation count is already zero".to_string(),
            ));
        }

        *guard -= 1;
        let now_zero = *guard == 0;
        if now_zero {
            self.peregrine
                .session
                .set_out_of_band_elicitation_pause_state(/*paused*/ false);
        }

        Ok(*guard)
    }
}
