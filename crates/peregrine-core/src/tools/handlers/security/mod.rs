mod capture;
mod model;
mod spec;

use self::model::*;
use crate::context::{AuditRunContextFragment, ContextualUserFragment};
use crate::function_tool::FunctionCallError;
use crate::session::turn_context::TurnContext;
use crate::tools::context::{FunctionToolOutput, ToolInvocation, ToolPayload, boxed_tool_output};
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::{CoreToolRuntime, ToolExecutor};
pub(crate) use capture::router_evidence_capture;
use chrono::Utc;
use codex_tools::{ToolName, ToolSpec};
use peregrine_audit_store::AuditStore;
use peregrine_types::{AuditEvidenceAttestation, AuditRun, AuditWorkItemStatus};
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

pub use spec::{
    CLAIM_AGENT_ASSIGNMENT_TOOL_NAME, CLAIM_WORK_TOOL_NAME, FINALIZE_REPORT_TOOL_NAME,
    FINISH_AGENT_ASSIGNMENT_TOOL_NAME, FINISH_WORK_TOOL_NAME, READ_RUN_TOOL_NAME,
    RECORD_AGENT_CONCLUSION_TOOL_NAME, RECORD_EVIDENCE_TOOL_NAME, RECORD_PACKET_TOOL_NAME,
    SET_AGENT_ASSIGNMENT_THREAD_TOOL_NAME,
};

const MAX_ID_BYTES: usize = 256;
const MAX_SUMMARY_BYTES: usize = 2_000;
const MAX_OBSERVATION_BYTES: usize = 8_000;
const MAX_PACKET_BYTES: usize = 128 * 1024;
const MAX_REFS: usize = 32;
const MAX_SCHEDULER_BLOCKS: usize = 64;
const SCHEDULER_WORKER_ID: &str = "audit-deterministic-scheduler";

#[derive(Clone, Copy)]
pub(crate) enum AuditToolHandler {
    ReadRun,
    ClaimWork,
    ClaimAgentAssignment,
    SetAgentAssignmentThread,
    FinishAgentAssignment,
    RecordPacket,
    RecordEvidence,
    RecordAgentConclusion,
    FinishWork,
    FinalizeReport,
}

impl AuditToolHandler {
    pub(crate) const ALL: [Self; 10] = [
        Self::ReadRun,
        Self::ClaimWork,
        Self::ClaimAgentAssignment,
        Self::SetAgentAssignmentThread,
        Self::FinishAgentAssignment,
        Self::RecordPacket,
        Self::RecordEvidence,
        Self::RecordAgentConclusion,
        Self::FinishWork,
        Self::FinalizeReport,
    ];
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for AuditToolHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(match self {
            Self::ReadRun => READ_RUN_TOOL_NAME,
            Self::ClaimWork => CLAIM_WORK_TOOL_NAME,
            Self::ClaimAgentAssignment => CLAIM_AGENT_ASSIGNMENT_TOOL_NAME,
            Self::SetAgentAssignmentThread => SET_AGENT_ASSIGNMENT_THREAD_TOOL_NAME,
            Self::FinishAgentAssignment => FINISH_AGENT_ASSIGNMENT_TOOL_NAME,
            Self::RecordPacket => RECORD_PACKET_TOOL_NAME,
            Self::RecordEvidence => RECORD_EVIDENCE_TOOL_NAME,
            Self::RecordAgentConclusion => RECORD_AGENT_CONCLUSION_TOOL_NAME,
            Self::FinishWork => FINISH_WORK_TOOL_NAME,
            Self::FinalizeReport => FINALIZE_REPORT_TOOL_NAME,
        })
    }

    fn spec(&self) -> ToolSpec {
        match self {
            Self::ReadRun => spec::read_run_tool(),
            Self::ClaimWork => spec::claim_work_tool(),
            Self::ClaimAgentAssignment => spec::claim_agent_assignment_tool(),
            Self::SetAgentAssignmentThread => spec::set_agent_assignment_thread_tool(),
            Self::FinishAgentAssignment => spec::finish_agent_assignment_tool(),
            Self::RecordPacket => spec::record_packet_tool(),
            Self::RecordEvidence => spec::record_evidence_tool(),
            Self::RecordAgentConclusion => spec::record_agent_conclusion_tool(),
            Self::FinishWork => spec::finish_work_tool(),
            Self::FinalizeReport => spec::finalize_report_tool(),
        }
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;
        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "audit handler received unsupported payload".to_string(),
                ));
            }
        };
        let scope = audit_scope(&turn).ok_or_else(|| {
            model_error("audit tools are only available in a persisted audit coordinator workspace")
        })?;
        let store = AuditStore::open(&scope.peregrine_home)
            .map_err(|error| FunctionCallError::Fatal(error.to_string()))?;
        let now = Utc::now().timestamp();

        let output = match self {
            Self::ReadRun => {
                let _: EmptyArgs = parse_arguments(&arguments)?;
                let run = read_run(&store, &scope.audit_id)?;
                json_output(&RunSummary::from_run(&run))?
            }
            Self::ClaimWork => {
                let args: ClaimWorkArgs = parse_arguments(&arguments)?;
                validate_text("worker_id", &args.worker_id, MAX_ID_BYTES)?;
                let scheduler_result =
                    block_unavailable_scheduled_work(&store, &scope.audit_id, now)?;
                let claimed = store
                    .claim_work(&scope.audit_id, &args.worker_id, None, now)
                    .map_err(tool_error)?;
                match claimed {
                    Some((run, work_item)) => {
                        inject_audit_context(&session, &turn, &run).await;
                        json_output(&ClaimWorkResponse {
                            agent_assignments: agent_assignments_for_work(&run, &work_item.id),
                            scheduler_blocks: scheduler_result.blocks,
                            work_item: WorkSummary::from(&work_item),
                            remaining_pending: pending_count(&run),
                        })?
                    }
                    None => {
                        if let Some(run) = &scheduler_result.latest_run {
                            inject_audit_context(&session, &turn, run).await;
                        }
                        json_output(&NoPendingWorkResponse {
                            message: "no pending work".to_string(),
                            scheduler_blocks: scheduler_result.blocks,
                        })?
                    }
                }
            }
            Self::ClaimAgentAssignment => {
                let args: ClaimAgentAssignmentArgs = parse_arguments(&arguments)?;
                validate_text("work_item_id", &args.work_item_id, MAX_ID_BYTES)?;
                validate_text("assignment_id", &args.assignment_id, MAX_ID_BYTES)?;
                validate_text("worker_id", &args.worker_id, MAX_ID_BYTES)?;
                let update = store
                    .claim_agent_assignment(
                        &scope.audit_id,
                        &args.work_item_id,
                        &args.assignment_id,
                        &args.worker_id,
                        now,
                    )
                    .map_err(tool_error)?;
                inject_audit_context(&session, &turn, &update.run).await;
                json_output(&AgentAssignmentResponse {
                    assignment: AgentAssignmentSummary::from(&update.assignment),
                })?
            }
            Self::SetAgentAssignmentThread => {
                let args: SetAgentAssignmentThreadArgs = parse_arguments(&arguments)?;
                validate_text("work_item_id", &args.work_item_id, MAX_ID_BYTES)?;
                validate_text("assignment_id", &args.assignment_id, MAX_ID_BYTES)?;
                validate_text("agent_thread_id", &args.agent_thread_id, MAX_ID_BYTES)?;
                let update = store
                    .update_agent_assignment_thread(
                        &scope.audit_id,
                        &args.work_item_id,
                        &args.assignment_id,
                        &args.agent_thread_id,
                        now,
                    )
                    .map_err(tool_error)?;
                inject_audit_context(&session, &turn, &update.run).await;
                json_output(&AgentAssignmentResponse {
                    assignment: AgentAssignmentSummary::from(&update.assignment),
                })?
            }
            Self::FinishAgentAssignment => {
                let args: FinishAgentAssignmentArgs = parse_arguments(&arguments)?;
                validate_text("work_item_id", &args.work_item_id, MAX_ID_BYTES)?;
                validate_text("assignment_id", &args.assignment_id, MAX_ID_BYTES)?;
                validate_text("reason", &args.reason, MAX_SUMMARY_BYTES)?;
                let update = store
                    .finish_agent_assignment(
                        &scope.audit_id,
                        &args.work_item_id,
                        &args.assignment_id,
                        args.status,
                        &args.reason,
                        now,
                    )
                    .map_err(tool_error)?;
                inject_audit_context(&session, &turn, &update.run).await;
                json_output(&AgentAssignmentResponse {
                    assignment: AgentAssignmentSummary::from(&update.assignment),
                })?
            }
            Self::RecordPacket => {
                let args: RecordPacketArgs = parse_arguments(&arguments)?;
                validate_text("work_item_id", &args.work_item_id, MAX_ID_BYTES)?;
                validate_text("packet_kind", &args.packet_kind, MAX_ID_BYTES)?;
                validate_text("summary", &args.summary, MAX_SUMMARY_BYTES)?;
                if !args.packet.is_object() {
                    return Err(model_error("packet must be a JSON object"));
                }
                validate_serialized_size("packet", &args.packet, MAX_PACKET_BYTES)?;
                let (_, artifact_ref) = store
                    .record_packet(
                        &scope.audit_id,
                        &args.work_item_id,
                        &args.packet_kind,
                        &args.summary,
                        args.packet,
                        now,
                    )
                    .map_err(tool_error)?;
                json_output(&ArtifactResponse { artifact_ref })?
            }
            Self::RecordEvidence => {
                let args: RecordEvidenceArgs = parse_arguments(&arguments)?;
                args.validate(&scope)?;
                let evidence = args.into_evidence(scope.audit_id.clone(), now);
                let (_, evidence_ref) = store
                    .record_evidence(&scope.audit_id, evidence)
                    .map_err(tool_error)?;
                json_output(&EvidenceResponse {
                    evidence_ref,
                    attestation: AuditEvidenceAttestation::ModelSubmitted,
                })?
            }
            Self::RecordAgentConclusion => {
                let args: RecordAgentConclusionArgs = parse_arguments(&arguments)?;
                args.validate(&scope)?;
                let work_item_id = args.work_item_id.clone();
                let conclusion = args.into_conclusion(scope.audit_id.clone(), now);
                let (_, artifact_ref) = store
                    .record_agent_conclusion(&scope.audit_id, &work_item_id, conclusion)
                    .map_err(tool_error)?;
                json_output(&ArtifactResponse { artifact_ref })?
            }
            Self::FinishWork => {
                let args: FinishWorkArgs = parse_arguments(&arguments)?;
                validate_text("work_item_id", &args.work_item_id, MAX_ID_BYTES)?;
                validate_text("worker_id", &args.worker_id, MAX_ID_BYTES)?;
                let evidence_refs = args.evidence_refs.unwrap_or_default();
                validate_refs(&scope, &evidence_refs, "evidence")?;
                let update = store
                    .finish_work(
                        &scope.audit_id,
                        &args.work_item_id,
                        &args.worker_id,
                        args.status,
                        &evidence_refs,
                        now,
                    )
                    .map_err(tool_error)?;
                if update.stage_changed {
                    inject_audit_context(&session, &turn, &update.run).await;
                }
                json_output(&FinishWorkResponse {
                    agent_assignments: agent_assignments_for_work(
                        &update.run,
                        &update.work_item.id,
                    ),
                    work_item: WorkSummary::from(&update.work_item),
                    current_stage: update.run.current_stage.clone(),
                    remaining_pending: pending_count(&update.run),
                })?
            }
            Self::FinalizeReport => {
                let args: FinalizeReportArgs = parse_arguments(&arguments)?;
                validate_serialized_size("report findings", &args, MAX_PACKET_BYTES)?;
                for finding in &args.findings {
                    validate_refs(&scope, &finding.evidence_refs, "evidence")?;
                }
                let finalized = store
                    .finalize_report(
                        &scope.audit_id,
                        args.findings,
                        args.metadata.unwrap_or_default(),
                        now,
                    )
                    .map_err(tool_error)?;
                inject_audit_context(&session, &turn, &finalized.run).await;
                json_output(&FinalizeReportResponse {
                    run: RunSummary::from_run(&finalized.run),
                    report_ref: finalized.report_ref,
                    markdown_ref: finalized.markdown_ref,
                })?
            }
        };
        Ok(boxed_tool_output(output))
    }
}

impl CoreToolRuntime for AuditToolHandler {}

pub(crate) fn audit_tools_enabled(turn: &TurnContext) -> bool {
    audit_scope(turn).is_some()
}

pub(crate) struct AuditScope {
    pub(crate) peregrine_home: PathBuf,
    audit_root: PathBuf,
    pub(crate) audit_id: String,
}

pub(crate) fn audit_scope(turn: &TurnContext) -> Option<AuditScope> {
    let cwd = turn
        .environments
        .primary()
        .map(|environment| environment.cwd.as_path())
        .unwrap_or_else(|| {
            #[allow(deprecated)]
            turn.cwd.as_path()
        });
    let audits_root = turn.config.peregrine_home.join("audits");
    let relative = cwd.strip_prefix(&audits_root).ok()?;
    let mut components = relative.components();
    let Component::Normal(audit_id) = components.next()? else {
        return None;
    };
    if components.next()? != Component::Normal("workspace".as_ref()) || components.next().is_some()
    {
        return None;
    }
    let audit_id = audit_id.to_str()?.to_string();
    Some(AuditScope {
        peregrine_home: turn.config.peregrine_home.to_path_buf(),
        audit_root: audits_root.join(&audit_id).to_path_buf(),
        audit_id,
    })
}

fn read_run(store: &AuditStore, audit_id: &str) -> Result<AuditRun, FunctionCallError> {
    store
        .read_run(audit_id)
        .map_err(tool_error)?
        .ok_or_else(|| model_error("persisted audit run was not found"))
}

async fn inject_audit_context(
    session: &crate::session::session::Session,
    turn: &TurnContext,
    run: &AuditRun,
) {
    session
        .inject_no_new_turn(
            vec![ContextualUserFragment::into(
                AuditRunContextFragment::from_run(run),
            )],
            Some(turn),
        )
        .await;
}

fn json_output(value: &impl Serialize) -> Result<FunctionToolOutput, FunctionCallError> {
    serde_json::to_string_pretty(value)
        .map(|value| FunctionToolOutput::from_text(value, Some(true)))
        .map_err(|error| FunctionCallError::Fatal(error.to_string()))
}

fn tool_error(error: peregrine_audit_store::AuditStoreError) -> FunctionCallError {
    model_error(&error.to_string())
}

fn model_error(message: &str) -> FunctionCallError {
    FunctionCallError::RespondToModel(message.to_string())
}

fn validate_text(name: &str, value: &str, max_bytes: usize) -> Result<(), FunctionCallError> {
    if value.is_empty() {
        return Err(model_error(&format!("{name} must not be empty")));
    }
    if value.len() > max_bytes {
        return Err(model_error(&format!(
            "{name} exceeds the {max_bytes}-byte limit"
        )));
    }
    Ok(())
}

fn validate_serialized_size(
    name: &str,
    value: &impl Serialize,
    max_bytes: usize,
) -> Result<(), FunctionCallError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| FunctionCallError::Fatal(error.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(model_error(&format!(
            "{name} exceeds the {max_bytes}-byte limit"
        )));
    }
    Ok(())
}

fn validate_refs(
    scope: &AuditScope,
    refs: &[String],
    required_prefix: &str,
) -> Result<(), FunctionCallError> {
    if refs.len() > MAX_REFS {
        return Err(model_error(&format!(
            "too many {required_prefix} references"
        )));
    }
    for value in refs {
        validate_text("artifact reference", value, MAX_ID_BYTES)?;
        let path = Path::new(value);
        let mut components = path.components();
        if components.next() != Some(Component::Normal(required_prefix.as_ref()))
            || components.any(|component| !matches!(component, Component::Normal(_)))
            || !scope.audit_root.join(path).is_file()
        {
            return Err(model_error(&format!(
                "artifact reference `{value}` is not an existing {required_prefix}/ file"
            )));
        }
    }
    Ok(())
}

fn pending_count(run: &AuditRun) -> usize {
    run.work_items
        .iter()
        .filter(|item| item.status == AuditWorkItemStatus::Pending)
        .count()
}

struct SchedulerBlockResult {
    blocks: Vec<SchedulerBlockSummary>,
    latest_run: Option<AuditRun>,
}

fn block_unavailable_scheduled_work(
    store: &AuditStore,
    audit_id: &str,
    now: i64,
) -> Result<SchedulerBlockResult, FunctionCallError> {
    let mut blocks = Vec::new();
    let mut latest_run = None;
    for _ in 0..MAX_SCHEDULER_BLOCKS {
        let Some(block) = store
            .block_next_unavailable_scheduled_work(audit_id, SCHEDULER_WORKER_ID, now)
            .map_err(tool_error)?
        else {
            return Ok(SchedulerBlockResult { blocks, latest_run });
        };
        latest_run = Some(block.run.clone());
        blocks.push(SchedulerBlockSummary::from(block));
    }
    Err(model_error(
        "deterministic audit scheduler reached its unavailable-stage limit",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_tool_set_includes_terminal_report_finalizer() {
        let names = AuditToolHandler::ALL
            .into_iter()
            .map(|handler| handler.tool_name().name)
            .collect::<Vec<_>>();

        assert!(names.contains(&CLAIM_AGENT_ASSIGNMENT_TOOL_NAME.to_string()));
        assert!(names.contains(&SET_AGENT_ASSIGNMENT_THREAD_TOOL_NAME.to_string()));
        assert!(names.contains(&FINISH_AGENT_ASSIGNMENT_TOOL_NAME.to_string()));
        assert!(names.contains(&FINALIZE_REPORT_TOOL_NAME.to_string()));
    }
}
