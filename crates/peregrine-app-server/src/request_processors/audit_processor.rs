use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::OutgoingMessageSender;
use crate::request_processors::ThreadGoalRequestProcessor;
use crate::request_processors::audit_support::coverage_gaps;
use crate::request_processors::audit_support::default_required_capabilities;
use crate::request_processors::audit_support::parse_coordinator_thread_id;
use crate::request_processors::audit_support::profile_from_params;
use crate::request_processors::audit_support::serialize;
use crate::request_processors::audit_support::target_from_params;
use crate::request_processors::audit_support::validate_profile;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_rollout::StateDbHandle;
use codex_utils_absolute_path::AbsolutePathBuf;
use peregrine_app_server_protocol::{
    AuditArtifactReadParams, AuditArtifactReadResponse, AuditCancelResponse, AuditDeleteResponse,
    AuditDiagnosticNotification, AuditLifecycleParams, AuditListParams, AuditListResponse,
    AuditPauseResponse, AuditPlanStoreParams, AuditPlanStoreResponse, AuditPreflightParams,
    AuditPreflightResponse, AuditReadParams, AuditReadResponse, AuditReportFormat,
    AuditReportReadParams, AuditReportReadResponse, AuditResumeResponse,
    AuditStageUpdatedNotification, AuditStartParams, AuditStartResponse, AuditUpdatedNotification,
    JSONRPCErrorError, ServerNotification,
};
use peregrine_audit_store::AuditStore;
use peregrine_core::agent_role_catalog::{
    AgentRoleCatalogEntry, AgentRoleCatalogSource, list_agent_roles,
};
use peregrine_core::config::Config;
use peregrine_core::context::AuditRunContextFragment;
use peregrine_core::context::ContextualUserFragment;
use peregrine_core::{ExternalGoalPreviousStatus, ExternalGoalSet, PeregrineThread, ThreadManager};
use peregrine_security_tools::{
    AuditAdapterRegistry, AuditWorkspace, attach_stage_schedules, create_audit_agent_assignments,
    create_audit_work_items, default_audit_stages,
};
use peregrine_types::{
    AuditAgentAssignment, AuditPlan, AuditProfile, AuditRun, AuditRunStatus, AuditStageId,
    AuditStageStatus, Metadata,
};
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

type AuditContinuationFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

/// Drives a persisted audit coordinator thread after lifecycle changes.
trait AuditCoordinatorContinuation: Send + Sync {
    fn continue_if_idle<'a>(&'a self, thread: &'a PeregrineThread) -> AuditContinuationFuture<'a>;
}

#[derive(Default)]
struct CoreAuditCoordinatorContinuation;

impl AuditCoordinatorContinuation for CoreAuditCoordinatorContinuation {
    fn continue_if_idle<'a>(&'a self, thread: &'a PeregrineThread) -> AuditContinuationFuture<'a> {
        Box::pin(thread.continue_active_goal_if_idle())
    }
}

#[derive(Clone)]
pub(crate) struct AuditRequestProcessor {
    store: Result<Arc<AuditStore>, String>,
    adapters: Arc<AuditAdapterRegistry>,
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    thread_goal_processor: ThreadGoalRequestProcessor,
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    state_db: Option<StateDbHandle>,
    coordinator_continuation: Arc<dyn AuditCoordinatorContinuation>,
}

impl AuditRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        thread_goal_processor: ThreadGoalRequestProcessor,
        outgoing: Arc<OutgoingMessageSender>,
        config: Arc<Config>,
        state_db: Option<StateDbHandle>,
        adapters: Arc<AuditAdapterRegistry>,
    ) -> Self {
        let store = AuditStore::open(&config.peregrine_home)
            .map(Arc::new)
            .map_err(|error| error.to_string());
        Self {
            store,
            adapters,
            auth_manager,
            thread_manager,
            thread_goal_processor,
            outgoing,
            config,
            state_db,
            coordinator_continuation: Arc::new(CoreAuditCoordinatorContinuation),
        }
    }

    #[cfg(test)]
    fn with_coordinator_continuation_for_tests(
        mut self,
        coordinator_continuation: Arc<dyn AuditCoordinatorContinuation>,
    ) -> Self {
        self.coordinator_continuation = coordinator_continuation;
        self
    }

    pub(crate) async fn preflight(
        &self,
        params: AuditPreflightParams,
    ) -> Result<AuditPreflightResponse, JSONRPCErrorError> {
        let target = target_from_params(params.target);
        let adapter = self
            .adapters
            .get(target.chain_id())
            .map_err(|error| invalid_request(error.to_string()))?;
        let preflight = adapter
            .preflight(&target)
            .await
            .map_err(|error| invalid_request(error.to_string()))?;
        let profile = params
            .profile
            .map_or_else(AuditProfile::default, profile_from_params);
        validate_profile(&profile)?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let plan = AuditPlan {
            schema_version: 1,
            id: Uuid::now_v7().to_string(),
            fingerprint: String::new(),
            target: preflight.normalized_target,
            profile,
            stages: default_audit_stages(),
            required_capabilities: default_required_capabilities(),
            created_at: now,
            metadata: Metadata::new(),
        };
        let diagnostics = preflight
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect();
        Ok(AuditPreflightResponse {
            plan: serialize(&plan)?,
            diagnostics,
        })
    }

    pub(crate) async fn store_plan(
        &self,
        params: AuditPlanStoreParams,
    ) -> Result<AuditPlanStoreResponse, JSONRPCErrorError> {
        let plan: AuditPlan = serde_json::from_value(params.plan)
            .map_err(|error| invalid_request(format!("invalid audit plan: {error}")))?;
        validate_profile(&plan.profile)?;
        let plan = self
            .store()?
            .store_plan(plan)
            .map_err(|error| internal_error(error.to_string()))?;
        Ok(AuditPlanStoreResponse {
            fingerprint: plan.fingerprint.clone(),
            plan: serialize(&plan)?,
        })
    }

    pub(crate) async fn start(
        &self,
        params: AuditStartParams,
    ) -> Result<AuditStartResponse, JSONRPCErrorError> {
        if !self.config.features.enabled(Feature::Goals) {
            return Err(invalid_request("goals feature is disabled"));
        }
        let store = self.store()?;
        let plan = store
            .read_plan(&params.fingerprint)
            .map_err(|error| internal_error(error.to_string()))?
            .ok_or_else(|| invalid_request("audit plan fingerprint was not found"))?;
        if plan.fingerprint != params.fingerprint {
            return Err(invalid_request("audit plan fingerprint mismatch"));
        }
        validate_profile(&plan.profile)?;
        let adapter = self
            .adapters
            .get(plan.target.chain_id())
            .map_err(|error| invalid_request(error.to_string()))?;
        let preflight = adapter
            .preflight(&plan.target)
            .await
            .map_err(|error| invalid_request(error.to_string()))?;
        let audit_id = Uuid::now_v7().to_string();
        let workspace = store
            .create_workspace(&audit_id)
            .map_err(|error| internal_error(error.to_string()))?;
        let acquired = adapter
            .acquire(&preflight.normalized_target, &plan.profile, &workspace)
            .await
            .map_err(|error| invalid_request(error.to_string()))?;
        let capabilities = preflight.capabilities;
        let coverage_gaps = coverage_gaps(&plan, &capabilities);
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let mut work_items = create_audit_work_items(&audit_id, &plan.stages, now);
        attach_stage_schedules(&mut work_items, &capabilities)
            .map_err(|error| internal_error(error.to_string()))?;
        let mut agent_assignments = create_audit_agent_assignments(&audit_id, &work_items, now);
        let agent_role_bindings =
            bind_audit_agent_roles(self.config.as_ref(), &mut agent_assignments)?;
        let mut metadata = Metadata::new();
        metadata.insert("acquiredTarget".to_string(), serialize(&acquired)?);
        metadata.insert("agentRoleBindings".to_string(), agent_role_bindings);
        let mut artifact_refs = vec![acquired.manifest_ref.clone()];
        artifact_refs.extend(acquired.artifact_refs.clone());
        let mut run = AuditRun {
            schema_version: 1,
            id: audit_id,
            plan_fingerprint: plan.fingerprint.clone(),
            target: preflight.normalized_target,
            profile: plan.profile,
            status: AuditRunStatus::Running,
            current_stage: plan
                .stages
                .first()
                .cloned()
                .unwrap_or(AuditStageId::AuditSession),
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: Some(adapter.adapter_id().to_string()),
            capabilities,
            coverage_gaps,
            work_items,
            agent_assignments,
            evidence_refs: Vec::new(),
            artifact_refs,
            created_at: now,
            updated_at: now,
            metadata,
        };
        let thread = self.start_coordinator(&workspace).await?;
        run.coordinator_thread_id = Some(thread.thread_id.to_string());
        store
            .create_run(&run)
            .map_err(|error| internal_error(error.to_string()))?;
        let objective = audit_coordinator_objective(&run.id);
        let goal_id = self
            .thread_goal_processor
            .create_goal_for_running_thread(
                thread.thread_id,
                thread.thread.as_ref(),
                &objective,
                Some(run.profile.model_token_budget),
            )
            .await?;
        run.goal_id = Some(goal_id);
        run.updated_at = OffsetDateTime::now_utc().unix_timestamp();
        store
            .update_run(&run)
            .map_err(|error| internal_error(error.to_string()))?;
        thread
            .thread
            .inject_response_items(vec![ContextualUserFragment::into(
                AuditRunContextFragment::from_run(&run),
            )])
            .await
            .map_err(|error| internal_error(format!("failed to inject audit context: {error}")))?;
        self.emit_updated(&run).await?;
        self.emit_stage_updated(&run, AuditStageStatus::Running)
            .await?;
        for gap in &run.coverage_gaps {
            self.outgoing
                .send_server_notification(ServerNotification::AuditDiagnostic(
                    AuditDiagnosticNotification {
                        audit_id: Some(run.id.clone()),
                        message: format!("{} unavailable: {}", gap.capability, gap.reason),
                    },
                ))
                .await;
        }
        self.continue_coordinator_goal(&run, thread.thread.as_ref())
            .await;
        Ok(AuditStartResponse {
            run: serialize(&run)?,
        })
    }

    pub(crate) async fn read(
        &self,
        params: AuditReadParams,
    ) -> Result<AuditReadResponse, JSONRPCErrorError> {
        let run = self.read_run(&params.audit_id)?;
        Ok(AuditReadResponse {
            run: serialize(&run)?,
        })
    }

    pub(crate) async fn list(
        &self,
        params: AuditListParams,
    ) -> Result<AuditListResponse, JSONRPCErrorError> {
        let offset = match params.cursor {
            Some(cursor) => cursor
                .parse::<u32>()
                .map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))?,
            None => 0,
        };
        let page = self
            .store()?
            .list_runs_page(offset, params.limit.unwrap_or(50))
            .map_err(|error| internal_error(error.to_string()))?;
        let data = page
            .runs
            .iter()
            .map(serialize)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AuditListResponse {
            data,
            next_cursor: page.next_cursor,
        })
    }

    pub(crate) async fn pause(
        &self,
        params: AuditLifecycleParams,
    ) -> Result<AuditPauseResponse, JSONRPCErrorError> {
        let run = self
            .transition(params.audit_id, AuditRunStatus::Paused)
            .await?;
        Ok(AuditPauseResponse {
            run: serialize(&run)?,
        })
    }

    pub(crate) async fn resume(
        &self,
        params: AuditLifecycleParams,
    ) -> Result<AuditResumeResponse, JSONRPCErrorError> {
        let run = self
            .transition(params.audit_id, AuditRunStatus::Running)
            .await?;
        Ok(AuditResumeResponse {
            run: serialize(&run)?,
        })
    }

    pub(crate) async fn cancel(
        &self,
        params: AuditLifecycleParams,
    ) -> Result<AuditCancelResponse, JSONRPCErrorError> {
        let run = self
            .transition(params.audit_id, AuditRunStatus::Cancelled)
            .await?;
        Ok(AuditCancelResponse {
            run: serialize(&run)?,
        })
    }

    pub(crate) async fn delete(
        &self,
        params: AuditLifecycleParams,
    ) -> Result<AuditDeleteResponse, JSONRPCErrorError> {
        let run = self.read_run(&params.audit_id)?;
        if matches!(run.status, AuditRunStatus::Running) {
            return Err(invalid_request(
                "pause or cancel a running audit before deletion",
            ));
        }
        let deleted = self
            .store()?
            .delete_run(&params.audit_id)
            .map_err(|error| internal_error(error.to_string()))?;
        Ok(AuditDeleteResponse { deleted })
    }

    pub(crate) async fn read_report(
        &self,
        params: AuditReportReadParams,
    ) -> Result<AuditReportReadResponse, JSONRPCErrorError> {
        let format = params.format.unwrap_or(AuditReportFormat::Markdown);
        let artifact_ref = match format {
            AuditReportFormat::Json => "reports/report.json",
            AuditReportFormat::Markdown => "reports/report.md",
        };
        let run = self.read_run(&params.audit_id)?;
        if !run.artifact_refs.iter().any(|known| known == artifact_ref) {
            return Err(invalid_request(
                "terminal audit report is not available for this run",
            ));
        }
        let artifact = self.read_artifact_bytes(&run.id, artifact_ref)?;
        Ok(AuditReportReadResponse {
            audit_id: run.id,
            artifact_ref: artifact_ref.to_string(),
            format,
            content_type: content_type_for_artifact(artifact_ref).to_string(),
            data_base64: STANDARD.encode(&artifact),
            text: text_content(&artifact),
            size_bytes: artifact.len() as u64,
        })
    }

    pub(crate) async fn read_artifact(
        &self,
        params: AuditArtifactReadParams,
    ) -> Result<AuditArtifactReadResponse, JSONRPCErrorError> {
        let run = self.read_run(&params.audit_id)?;
        if !is_recorded_artifact_ref(&run, &params.artifact_ref) {
            return Err(invalid_request(
                "audit artifact ref was not recorded on this run",
            ));
        }
        let artifact = self.read_artifact_bytes(&run.id, &params.artifact_ref)?;
        Ok(AuditArtifactReadResponse {
            audit_id: run.id,
            artifact_ref: params.artifact_ref.clone(),
            content_type: content_type_for_artifact(&params.artifact_ref).to_string(),
            data_base64: STANDARD.encode(&artifact),
            text: text_content(&artifact),
            size_bytes: artifact.len() as u64,
        })
    }

    async fn start_coordinator(
        &self,
        workspace: &AuditWorkspace,
    ) -> Result<peregrine_core::NewThread, JSONRPCErrorError> {
        let config = self.coordinator_config(workspace)?;
        self.thread_manager
            .start_thread(config)
            .await
            .map_err(|error| internal_error(format!("failed to start audit coordinator: {error}")))
    }

    fn coordinator_config(&self, workspace: &AuditWorkspace) -> Result<Config, JSONRPCErrorError> {
        let mut config = self.config.as_ref().clone();
        let cwd = AbsolutePathBuf::from_absolute_path_checked(&workspace.workspace)
            .map_err(|error| internal_error(format!("invalid audit workspace path: {error}")))?;
        let writable_roots = [
            &workspace.workspace,
            &workspace.artifacts,
            &workspace.evidence,
            &workspace.traces,
            &workspace.reports,
        ]
        .into_iter()
        .map(AbsolutePathBuf::from_absolute_path_checked)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| internal_error(format!("invalid audit workspace path: {error}")))?;
        config.cwd = cwd;
        config.permissions.set_workspace_roots(writable_roots);
        config.ephemeral = false;
        let instructions = "You are the coordinator for a persisted autonomous security audit. \
Never modify immutable audit input. Use only registered tools and isolated audit workspace paths. \
Start coordinator work by calling audit_read_run, then audit_claim_work. \
For each claimed item, persist bounded stage packets with audit_record_packet, persist evidence with audit_record_evidence, and close the item with audit_finish_work. \
For claimed work with a scheduled capability, first call audit_prepare_capability. If it returns a concrete tool, call that ToolRouter-visible tool normally. If it returns a discovery query, use announced tools via tool_search or the current model-visible tool list; never import or call MCP server implementations directly. After the tool succeeds, call audit_read_run and confirm router-captured evidence is attached before completing capability-backed work. \
For claimed work that has agentAssignments, call audit_claim_agent_assignment before spawning each configured role with spawn_agent. Use the assignment roleName as agent_type and pass only public, bounded inputs: audit ID, work item ID, assignment ID, current stage, evidence refs, artifact refs, and recorded packet refs. After spawn_agent returns, call audit_set_agent_assignment_thread with the returned child agent task name or thread identifier, then call wait_agent when the role result is on the critical path. \
After wait_agent returns, call audit_read_run and, when needed, list_agents for the spawned task. If wait_agent times out, the agent reaches an error/interrupted/shutdown/not_found state, or no audit_record_agent_conclusion artifact appears for that assignment after a final agent response, call audit_finish_agent_assignment with failed or cancelled and a public reason. \
Run adversarial review as Researcher to Skeptic to Exploiter to Judge. The Judge must receive only normalized evidence, replay results, public role conclusions, and artifact refs; never share hidden reasoning between agents. \
Each audit agent must record its public conclusion with audit_record_agent_conclusion before its assignment is considered complete. If an agent cannot produce a conclusion, call audit_finish_agent_assignment with failed or cancelled and record a visible diagnostic in the stage packet. \
Never call audit_finish_work with completed while any assignment for that work item is pending, spawned, failed, or cancelled; failed or cancelled assignments mean the work item must become blocked or failed and later reports must show the gap. \
Continue through the persisted stage queue until audit_finalize_report succeeds, a budget or usage limit stops the goal, or a visible diagnostic-worthy blocker prevents progress. \
Treat generated code as a hypothesis until an isolated exploit replay produces evidence. \
Do not confirm findings without two independent verification classes and successful replay.";
        config.developer_instructions = Some(match config.developer_instructions {
            Some(existing) => format!("{existing}\n\n{instructions}"),
            None => instructions.to_string(),
        });
        Ok(config)
    }

    async fn transition(
        &self,
        audit_id: String,
        next_status: AuditRunStatus,
    ) -> Result<AuditRun, JSONRPCErrorError> {
        let mut run = self.read_run(&audit_id)?;
        let thread = self.coordinator_thread(&run).await?;
        let goal_status = match next_status {
            AuditRunStatus::Running => codex_state::ThreadGoalStatus::Active,
            AuditRunStatus::Paused => codex_state::ThreadGoalStatus::Paused,
            AuditRunStatus::Cancelled => codex_state::ThreadGoalStatus::Blocked,
            _ => return Err(invalid_request("unsupported audit lifecycle transition")),
        };
        self.update_linked_goal(&run, thread.as_ref(), goal_status)
            .await?;
        run.status = next_status;
        run.updated_at = OffsetDateTime::now_utc().unix_timestamp();
        self.store()?
            .update_run(&run)
            .map_err(|error| internal_error(error.to_string()))?;
        if matches!(run.status, AuditRunStatus::Running) {
            thread
                .inject_response_items(vec![ContextualUserFragment::into(
                    AuditRunContextFragment::from_run(&run),
                )])
                .await
                .map_err(|error| {
                    internal_error(format!("failed to inject audit context: {error}"))
                })?;
        }
        self.emit_updated(&run).await?;
        self.emit_stage_updated(&run, lifecycle_stage_status(&run.status))
            .await?;
        self.continue_coordinator_goal(&run, thread.as_ref()).await;
        Ok(run)
    }

    async fn continue_coordinator_goal(&self, run: &AuditRun, thread: &PeregrineThread) {
        if !matches!(run.status, AuditRunStatus::Running) {
            return;
        }
        if let Err(error) = self.coordinator_continuation.continue_if_idle(thread).await {
            tracing::warn!(
                audit_id = %run.id,
                "failed to continue audit coordinator goal: {error}"
            );
            self.outgoing
                .send_server_notification(ServerNotification::AuditDiagnostic(
                    AuditDiagnosticNotification {
                        audit_id: Some(run.id.clone()),
                        message: format!("audit coordinator continuation failed: {error}"),
                    },
                ))
                .await;
        }
    }

    async fn update_linked_goal(
        &self,
        run: &AuditRun,
        thread: &PeregrineThread,
        status: codex_state::ThreadGoalStatus,
    ) -> Result<(), JSONRPCErrorError> {
        let thread_id = parse_coordinator_thread_id(run)?;
        let state_db = thread
            .state_db()
            .or_else(|| self.state_db.clone())
            .ok_or_else(|| internal_error("goal persistence is unavailable"))?;
        let previous = state_db
            .thread_goals()
            .get_thread_goal(thread_id)
            .await
            .map_err(|error| internal_error(error.to_string()))?
            .ok_or_else(|| invalid_request("audit coordinator goal is missing"))?;
        let goal = state_db
            .thread_goals()
            .update_thread_goal(
                thread_id,
                codex_state::GoalUpdate {
                    objective: None,
                    status: Some(status),
                    token_budget: None,
                    expected_goal_id: run.goal_id.clone(),
                },
            )
            .await
            .map_err(|error| internal_error(error.to_string()))?
            .ok_or_else(|| invalid_request("audit coordinator goal is missing"))?;
        thread
            .apply_external_goal_set(ExternalGoalSet {
                goal,
                previous_status: ExternalGoalPreviousStatus::from(&previous),
            })
            .await;
        Ok(())
    }

    async fn coordinator_thread(
        &self,
        run: &AuditRun,
    ) -> Result<Arc<PeregrineThread>, JSONRPCErrorError> {
        let thread_id = parse_coordinator_thread_id(run)?;
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            return Ok(thread);
        }

        let workspace = self
            .store()?
            .create_workspace(&run.id)
            .map_err(|error| internal_error(error.to_string()))?;
        let config = self.coordinator_config(&workspace)?;
        let thread = self
            .thread_manager
            .resume_thread_from_store(config, thread_id, self.auth_manager.clone(), None)
            .await
            .map_err(|error| {
                invalid_request(format!(
                    "audit coordinator thread could not be loaded: {error}"
                ))
            })?;
        Ok(thread.thread)
    }

    fn read_run(&self, audit_id: &str) -> Result<AuditRun, JSONRPCErrorError> {
        self.store()?
            .read_run(audit_id)
            .map_err(|error| internal_error(error.to_string()))?
            .ok_or_else(|| invalid_request("audit run was not found"))
    }

    fn read_artifact_bytes(
        &self,
        audit_id: &str,
        artifact_ref: &str,
    ) -> Result<Vec<u8>, JSONRPCErrorError> {
        self.store()?
            .read_artifact(audit_id, artifact_ref)
            .map_err(|error| internal_error(error.to_string()))
    }

    fn store(&self) -> Result<&Arc<AuditStore>, JSONRPCErrorError> {
        self.store
            .as_ref()
            .map_err(|error| internal_error(format!("audit store is unavailable: {error}")))
    }

    async fn emit_updated(&self, run: &AuditRun) -> Result<(), JSONRPCErrorError> {
        self.outgoing
            .send_server_notification(ServerNotification::AuditUpdated(AuditUpdatedNotification {
                audit_id: run.id.clone(),
                run: serialize(run)?,
            }))
            .await;
        Ok(())
    }

    async fn emit_stage_updated(
        &self,
        run: &AuditRun,
        status: AuditStageStatus,
    ) -> Result<(), JSONRPCErrorError> {
        self.outgoing
            .send_server_notification(ServerNotification::AuditStageUpdated(
                AuditStageUpdatedNotification {
                    audit_id: run.id.clone(),
                    stage: serialize(&run.current_stage)?,
                    status: serialize(&status)?,
                    run: serialize(run)?,
                },
            ))
            .await;
        Ok(())
    }
}

fn lifecycle_stage_status(status: &AuditRunStatus) -> AuditStageStatus {
    match status {
        AuditRunStatus::Pending => AuditStageStatus::Pending,
        AuditRunStatus::Running => AuditStageStatus::Running,
        AuditRunStatus::Paused => AuditStageStatus::Blocked,
        AuditRunStatus::Completed | AuditRunStatus::CompletedWithGaps => {
            AuditStageStatus::Succeeded
        }
        AuditRunStatus::Failed => AuditStageStatus::Failed,
        AuditRunStatus::Cancelled => AuditStageStatus::Cancelled,
    }
}

fn audit_coordinator_objective(audit_id: &str) -> String {
    format!(
        "Complete audit {audit_id} using its persisted stage plan. Drive the deterministic audit queue through audit_read_run, audit_claim_work, audit_record_packet, audit_record_evidence, audit_finish_work, and audit_finalize_report. Use only ToolRouter-visible native, code-mode, and MCP capabilities. Never treat generated code, model analysis, or knowledge citations as evidence until registered tools persist normalized evidence. Only complete after a terminal audit report exists."
    )
}

fn bind_audit_agent_roles(
    config: &Config,
    assignments: &mut [AuditAgentAssignment],
) -> Result<Value, JSONRPCErrorError> {
    let catalog = list_agent_roles(config);
    let mut bindings = Vec::new();
    for assignment in assignments {
        let role = catalog
            .iter()
            .find(|entry| entry.name == assignment.role_name)
            .ok_or_else(|| {
                invalid_request(format!(
                    "audit agent role `{}` is not configured",
                    assignment.role_name
                ))
            })?;
        stamp_agent_assignment_role_metadata(assignment, role)?;
        bindings.push(agent_role_binding_value(assignment, role)?);
    }
    Ok(Value::Array(bindings))
}

fn stamp_agent_assignment_role_metadata(
    assignment: &mut AuditAgentAssignment,
    role: &AgentRoleCatalogEntry,
) -> Result<(), JSONRPCErrorError> {
    assignment.metadata.insert(
        "roleSource".to_string(),
        json!(agent_role_source_name(&role.source)),
    );
    assignment.metadata.insert(
        "roleOverridesBuiltIn".to_string(),
        json!(role.overrides_built_in),
    );
    if let Some(config_file) = &role.config_file {
        assignment.metadata.insert(
            "roleConfigFile".to_string(),
            json!(config_file.display().to_string()),
        );
    }
    if role.nickname_candidates.is_some() {
        assignment.metadata.insert(
            "roleNicknameCandidates".to_string(),
            serde_json::to_value(&role.nickname_candidates)
                .map_err(|error| internal_error(error.to_string()))?,
        );
    }
    Ok(())
}

fn agent_role_binding_value(
    assignment: &AuditAgentAssignment,
    role: &AgentRoleCatalogEntry,
) -> Result<Value, JSONRPCErrorError> {
    Ok(json!({
        "assignmentId": assignment.id,
        "workItemId": assignment.work_item_id,
        "role": serde_json::to_value(&assignment.role)
            .map_err(|error| internal_error(error.to_string()))?,
        "roleName": assignment.role_name,
        "source": agent_role_source_name(&role.source),
        "overridesBuiltIn": role.overrides_built_in,
        "configFile": role.config_file
            .as_ref()
            .map(|config_file| config_file.display().to_string()),
        "nicknameCandidates": role.nickname_candidates,
    }))
}

fn agent_role_source_name(source: &AgentRoleCatalogSource) -> &'static str {
    match source {
        AgentRoleCatalogSource::BuiltIn => "builtIn",
        AgentRoleCatalogSource::Configured => "configured",
    }
}

fn is_recorded_artifact_ref(run: &AuditRun, artifact_ref: &str) -> bool {
    run.artifact_refs.iter().any(|known| known == artifact_ref)
        || run.evidence_refs.iter().any(|known| known == artifact_ref)
}

fn content_type_for_artifact(artifact_ref: &str) -> &'static str {
    if artifact_ref.ends_with(".json") {
        "application/json"
    } else if artifact_ref.ends_with(".md") {
        "text/markdown"
    } else if artifact_ref.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

fn text_content(bytes: &[u8]) -> Option<String> {
    String::from_utf8(bytes.to_vec()).ok()
}

#[cfg(test)]
#[path = "audit_processor_tests.rs"]
mod tests;
