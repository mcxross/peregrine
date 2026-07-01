#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use super::{
    AuditScope, MAX_ID_BYTES, MAX_OBSERVATION_BYTES, MAX_SUMMARY_BYTES, model_error, validate_refs,
    validate_serialized_size, validate_text,
};
use crate::function_tool::FunctionCallError;
use peregrine_audit_store::ScheduledWorkBlock;
use peregrine_types::{
    AuditAgentAssignment, AuditAgentAssignmentStatus, AuditAgentConclusion,
    AuditAgentConclusionStatus, AuditAgentRole, AuditEvidence, AuditEvidenceAttestation, AuditPlan,
    AuditPlannerOutput, AuditProfile, AuditRun, AuditRunStatus, AuditStageId, AuditTarget,
    AuditWorkItem, AuditWorkItemStatus, FindingCandidate, Metadata, SourcePrecision,
    VerificationMethod,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub(super) struct EmptyArgs {}

#[derive(Deserialize)]
pub(super) struct StorePlanArgs {
    pub(super) target: AuditTarget,
    pub(super) profile: AuditProfile,
    pub(super) stages: Vec<AuditStageId>,
    pub(super) desired_capabilities: Vec<String>,
    pub(super) planner_output: AuditPlannerOutput,
}

#[derive(Serialize)]
pub(super) struct StorePlanResponse {
    pub(super) fingerprint: String,
    pub(super) start_command: String,
    pub(super) plan: AuditPlan,
}

#[derive(Deserialize)]
pub(super) struct ClaimWorkArgs {
    pub(super) worker_id: String,
}

#[derive(Deserialize)]
pub(super) struct ClaimAgentAssignmentArgs {
    pub(super) work_item_id: String,
    pub(super) assignment_id: String,
    pub(super) worker_id: String,
}

#[derive(Deserialize)]
pub(super) struct SetAgentAssignmentThreadArgs {
    pub(super) work_item_id: String,
    pub(super) assignment_id: String,
    pub(super) agent_thread_id: String,
}

#[derive(Deserialize)]
pub(super) struct FinishAgentAssignmentArgs {
    pub(super) work_item_id: String,
    pub(super) assignment_id: String,
    pub(super) status: AuditAgentAssignmentStatus,
    pub(super) reason: String,
}

#[derive(Deserialize)]
pub(super) struct RecordPacketArgs {
    pub(super) work_item_id: String,
    pub(super) packet_kind: String,
    pub(super) summary: String,
    pub(super) packet: Value,
}

#[derive(Deserialize)]
pub(super) struct RecordEvidenceArgs {
    pub(super) work_item_id: String,
    verification_method: VerificationMethod,
    provider_id: String,
    adapter_id: Option<String>,
    tool_name: String,
    tool_version: Option<String>,
    input_hash: String,
    source_precision: SourcePrecision,
    summary: String,
    observation: String,
    execution_trace_ref: Option<String>,
    artifact_refs: Option<Vec<String>>,
}

impl RecordEvidenceArgs {
    pub(super) fn validate(&self, scope: &AuditScope) -> Result<(), FunctionCallError> {
        for (name, value) in [
            ("work_item_id", self.work_item_id.as_str()),
            ("provider_id", self.provider_id.as_str()),
            ("tool_name", self.tool_name.as_str()),
            ("input_hash", self.input_hash.as_str()),
        ] {
            validate_text(name, value, MAX_ID_BYTES)?;
        }
        validate_text("summary", &self.summary, MAX_SUMMARY_BYTES)?;
        validate_text("observation", &self.observation, MAX_OBSERVATION_BYTES)?;
        if let Some(adapter_id) = &self.adapter_id {
            validate_text("adapter_id", adapter_id, MAX_ID_BYTES)?;
        }
        if let Some(tool_version) = &self.tool_version {
            validate_text("tool_version", tool_version, MAX_ID_BYTES)?;
        }
        validate_refs(
            scope,
            self.artifact_refs.as_deref().unwrap_or_default(),
            "artifacts",
        )?;
        if let Some(trace_ref) = &self.execution_trace_ref {
            validate_refs(scope, std::slice::from_ref(trace_ref), "traces")?;
        }
        if matches!(
            self.verification_method,
            VerificationMethod::GeneratedTest | VerificationMethod::ExploitReplay
        ) && self.execution_trace_ref.is_none()
        {
            return Err(model_error(
                "generated tests and exploit replays require an existing execution trace",
            ));
        }
        Ok(())
    }

    pub(super) fn into_evidence(self, audit_run_id: String, created_at: i64) -> AuditEvidence {
        AuditEvidence {
            id: String::new(),
            audit_run_id,
            work_item_id: Some(self.work_item_id),
            verification_method: self.verification_method,
            provider_id: self.provider_id,
            adapter_id: self.adapter_id,
            tool_name: self.tool_name,
            tool_version: self.tool_version,
            input_hash: self.input_hash,
            source_precision: self.source_precision,
            attestation: AuditEvidenceAttestation::ModelSubmitted,
            summary: self.summary,
            observation: self.observation,
            execution_trace_ref: self.execution_trace_ref,
            artifact_refs: self.artifact_refs.unwrap_or_default(),
            created_at,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct RecordAgentConclusionArgs {
    pub(super) work_item_id: String,
    role: AuditAgentRole,
    agent_thread_id: Option<String>,
    status: AuditAgentConclusionStatus,
    summary: String,
    candidate_ids: Option<Vec<String>>,
    evidence_refs: Option<Vec<String>>,
    artifact_refs: Option<Vec<String>>,
    metadata: Option<Metadata>,
}

impl RecordAgentConclusionArgs {
    pub(super) fn validate(&self, scope: &AuditScope) -> Result<(), FunctionCallError> {
        validate_text("work_item_id", &self.work_item_id, MAX_ID_BYTES)?;
        validate_text("summary", &self.summary, MAX_SUMMARY_BYTES)?;
        if let Some(agent_thread_id) = &self.agent_thread_id {
            validate_text("agent_thread_id", agent_thread_id, MAX_ID_BYTES)?;
        }
        let candidate_ids = self.candidate_ids.as_deref().unwrap_or_default();
        if candidate_ids.len() > super::MAX_REFS {
            return Err(model_error("too many candidate references"));
        }
        for candidate_id in candidate_ids {
            validate_text("candidate_id", candidate_id, MAX_ID_BYTES)?;
        }
        let evidence_refs = self.evidence_refs.as_deref().unwrap_or_default();
        validate_refs(scope, evidence_refs, "evidence")?;
        validate_refs(
            scope,
            self.artifact_refs.as_deref().unwrap_or_default(),
            "artifacts",
        )?;
        if self.role == AuditAgentRole::Judge
            && matches!(
                self.status,
                AuditAgentConclusionStatus::Accepted | AuditAgentConclusionStatus::Supported
            )
            && evidence_refs.is_empty()
        {
            return Err(model_error(
                "positive judge conclusions require persisted evidence references",
            ));
        }
        validate_serialized_size("agent conclusion", self, super::MAX_PACKET_BYTES)?;
        Ok(())
    }

    pub(super) fn into_conclusion(
        self,
        audit_run_id: String,
        created_at: i64,
    ) -> AuditAgentConclusion {
        AuditAgentConclusion {
            schema_version: 1,
            id: String::new(),
            audit_run_id,
            work_item_id: self.work_item_id,
            role: self.role,
            agent_thread_id: self.agent_thread_id,
            status: self.status,
            summary: self.summary,
            candidate_ids: self.candidate_ids.unwrap_or_default(),
            evidence_refs: self.evidence_refs.unwrap_or_default(),
            artifact_refs: self.artifact_refs.unwrap_or_default(),
            created_at,
            metadata: self.metadata.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
pub(super) struct PrepareCapabilityArgs {
    pub(super) work_item_id: String,
    pub(super) capability: String,
}

#[derive(Deserialize)]
pub(super) struct RecordCapabilityGapArgs {
    pub(super) work_item_id: String,
    pub(super) capability: String,
    pub(super) reason: String,
    pub(super) provider_id: Option<String>,
    pub(super) tool_name: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct FinishWorkArgs {
    pub(super) work_item_id: String,
    pub(super) worker_id: String,
    pub(super) status: AuditWorkItemStatus,
    pub(super) evidence_refs: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct FinalizeReportArgs {
    pub(super) findings: Vec<FindingCandidate>,
    pub(super) metadata: Option<Metadata>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunSummary {
    audit_id: String,
    status: AuditRunStatus,
    current_stage: AuditStageId,
    work_counts: BTreeMap<String, usize>,
    agent_counts: BTreeMap<String, usize>,
    next_work: Vec<WorkSummary>,
    agent_assignments: Vec<AgentAssignmentSummary>,
    evidence_count: usize,
    artifact_count: usize,
    coverage_gaps: Vec<String>,
}

impl RunSummary {
    pub(super) fn from_run(run: &AuditRun) -> Self {
        let mut work_counts = BTreeMap::new();
        for work_item in &run.work_items {
            *work_counts
                .entry(format!("{:?}", work_item.status))
                .or_insert(0) += 1;
        }
        let mut agent_counts = BTreeMap::new();
        for assignment in &run.agent_assignments {
            *agent_counts
                .entry(format!("{:?}", assignment.status))
                .or_insert(0) += 1;
        }
        Self {
            audit_id: run.id.clone(),
            status: run.status.clone(),
            current_stage: run.current_stage.clone(),
            work_counts,
            agent_counts,
            next_work: run
                .work_items
                .iter()
                .filter(|item| {
                    matches!(
                        item.status,
                        AuditWorkItemStatus::Pending | AuditWorkItemStatus::Claimed
                    )
                })
                .take(12)
                .map(WorkSummary::from)
                .collect(),
            agent_assignments: next_agent_assignments(run),
            evidence_count: run.evidence_refs.len(),
            artifact_count: run.artifact_refs.len(),
            coverage_gaps: run
                .coverage_gaps
                .iter()
                .take(12)
                .map(|gap| format!("{}: {}", gap.capability, gap.reason))
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentAssignmentSummary {
    id: String,
    work_item_id: String,
    role: AuditAgentRole,
    role_name: String,
    status: AuditAgentAssignmentStatus,
    agent_thread_id: Option<String>,
    conclusion_ref_count: usize,
}

impl From<&AuditAgentAssignment> for AgentAssignmentSummary {
    fn from(value: &AuditAgentAssignment) -> Self {
        Self {
            id: value.id.clone(),
            work_item_id: value.work_item_id.clone(),
            role: value.role.clone(),
            role_name: value.role_name.clone(),
            status: value.status.clone(),
            agent_thread_id: value.agent_thread_id.clone(),
            conclusion_ref_count: value.conclusion_refs.len(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkSummary {
    id: String,
    stage: AuditStageId,
    status: AuditWorkItemStatus,
    title: String,
    claimed_by: Option<String>,
    attempts: u32,
    evidence_count: usize,
    schedule: Option<WorkScheduleSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkScheduleSummary {
    action: String,
    desired_capabilities: Vec<String>,
    available_capabilities: Vec<String>,
    unavailable_capabilities: Vec<String>,
    verification_methods: Vec<String>,
}

impl From<&AuditWorkItem> for WorkSummary {
    fn from(value: &AuditWorkItem) -> Self {
        Self {
            id: value.id.clone(),
            stage: value.stage.clone(),
            status: value.status.clone(),
            title: value.title.clone(),
            claimed_by: value.claimed_by.clone(),
            attempts: value.attempts,
            evidence_count: value.evidence_refs.len(),
            schedule: work_schedule_summary(value),
        }
    }
}

fn work_schedule_summary(work_item: &AuditWorkItem) -> Option<WorkScheduleSummary> {
    let schedule = work_item.metadata.get("stageSchedule")?;
    Some(WorkScheduleSummary {
        action: schedule.get("action")?.as_str()?.to_string(),
        desired_capabilities: string_array(schedule.get("desiredCapabilities")),
        available_capabilities: capability_array(schedule.get("availableCapabilities")),
        unavailable_capabilities: unavailable_capability_array(
            schedule.get("unavailableCapabilities"),
        ),
        verification_methods: string_array(schedule.get("verificationMethods")),
    })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(super::MAX_REFS)
        .map(str::to_string)
        .collect()
}

fn capability_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.get("capability").and_then(Value::as_str))
        .take(super::MAX_REFS)
        .map(str::to_string)
        .collect()
}

fn unavailable_capability_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let capability = value.get("capability")?.as_str()?;
            let reason = value.get("reason")?.as_str()?;
            Some(format!("{capability}: {reason}"))
        })
        .take(super::MAX_REFS)
        .collect()
}

fn next_agent_assignments(run: &AuditRun) -> Vec<AgentAssignmentSummary> {
    let pending_work_ids = run
        .work_items
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                AuditWorkItemStatus::Pending | AuditWorkItemStatus::Claimed
            )
        })
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    let mut assignments = run
        .agent_assignments
        .iter()
        .filter(|assignment| {
            pending_work_ids
                .iter()
                .any(|work_item_id| *work_item_id == assignment.work_item_id)
        })
        .take(12)
        .map(AgentAssignmentSummary::from)
        .collect::<Vec<_>>();
    if !assignments.is_empty() {
        return assignments;
    }

    assignments = run
        .agent_assignments
        .iter()
        .filter(|assignment| assignment.status == AuditAgentAssignmentStatus::Pending)
        .take(12)
        .map(AgentAssignmentSummary::from)
        .collect::<Vec<_>>();
    if !assignments.is_empty() {
        return assignments;
    }

    run.agent_assignments
        .iter()
        .take(12)
        .map(AgentAssignmentSummary::from)
        .collect()
}

pub(super) fn agent_assignments_for_work(
    run: &AuditRun,
    work_item_id: &str,
) -> Vec<AgentAssignmentSummary> {
    run.agent_assignments
        .iter()
        .filter(|assignment| assignment.work_item_id == work_item_id)
        .take(12)
        .map(AgentAssignmentSummary::from)
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClaimWorkResponse {
    pub(super) work_item: WorkSummary,
    pub(super) agent_assignments: Vec<AgentAssignmentSummary>,
    pub(super) scheduler_blocks: Vec<SchedulerBlockSummary>,
    pub(super) remaining_pending: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NoPendingWorkResponse {
    pub(super) message: String,
    pub(super) scheduler_blocks: Vec<SchedulerBlockSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SchedulerBlockSummary {
    work_item_id: String,
    stage: AuditStageId,
    artifact_ref: String,
    diagnostics: Vec<String>,
}

impl From<ScheduledWorkBlock> for SchedulerBlockSummary {
    fn from(value: ScheduledWorkBlock) -> Self {
        Self {
            work_item_id: value.work_item.id,
            stage: value.work_item.stage,
            artifact_ref: value.artifact_ref,
            diagnostics: value.diagnostics,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapabilityDispatchResponse {
    pub(super) artifact_ref: String,
    pub(super) dispatch: CapabilityDispatch,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapabilityDispatch {
    schema_version: u8,
    work_item_id: String,
    stage: AuditStageId,
    capability: String,
    provider_id: String,
    adapter_id: Option<String>,
    tool_name: Option<String>,
    tool_namespace: Option<String>,
    tool_function: Option<String>,
    discovery_query: Option<String>,
    target_context: Value,
    expected_verification_methods: Vec<String>,
    reducer: CapabilityReducer,
    instructions: String,
}

impl CapabilityDispatch {
    pub(super) fn packet(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|error| {
            serde_json::json!({
                "schemaVersion": 1,
                "serializationError": error.to_string(),
            })
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityReducer {
    evidence_attestation: AuditEvidenceAttestation,
    accepted_evidence_source: String,
    note: String,
}

pub(super) fn prepare_capability_dispatch(
    run: &AuditRun,
    args: &PrepareCapabilityArgs,
) -> Result<CapabilityDispatch, FunctionCallError> {
    let work_item = run
        .work_items
        .iter()
        .find(|work_item| work_item.id == args.work_item_id)
        .ok_or_else(|| model_error("work item was not found"))?;
    if work_item.status != AuditWorkItemStatus::Claimed {
        return Err(model_error(
            "work item must be claimed before preparing a capability",
        ));
    }
    let schedule = work_item
        .metadata
        .get("stageSchedule")
        .ok_or_else(|| model_error("work item has no stage schedule"))?;
    let desired_capabilities = string_array(schedule.get("desiredCapabilities"));
    if !desired_capabilities
        .iter()
        .any(|capability| capability == &args.capability)
    {
        return Err(model_error(
            "requested capability is not desired for this work item",
        ));
    }
    let binding = schedule
        .get("availableCapabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|binding| {
            binding.get("capability").and_then(Value::as_str) == Some(args.capability.as_str())
        })
        .cloned();
    let package_root = acquired_package_root(run)?;
    let tool_name = binding
        .as_ref()
        .and_then(|binding| binding.get("toolName").and_then(Value::as_str));
    let (tool_namespace, tool_function) = tool_name.map(split_tool_name).unwrap_or((None, None));
    let discovery_query = tool_name.is_none().then(|| {
        run.adapter_id.as_deref().map_or_else(
            || format!("{} audit capability", args.capability),
            |adapter_id| format!("{adapter_id} {} audit capability", args.capability),
        )
    });
    let instructions = if tool_name.is_some() {
        "Call the returned ToolRouter-visible tool using its visible schema and the returned targetContext. After the tool succeeds, call audit_read_run to confirm router-captured evidence was attached before audit_finish_work."
    } else {
        "No concrete tool name was bound into the schedule. Discover an announced ToolRouter-visible tool for this capability using tool_search or the current model-visible MCP/native tool list, then call that tool using its visible schema and the returned targetContext. Do not import or call an MCP server implementation directly. After the tool succeeds, call audit_read_run to confirm router-captured evidence was attached before audit_finish_work."
    };
    Ok(CapabilityDispatch {
        schema_version: 1,
        work_item_id: work_item.id.clone(),
        stage: work_item.stage.clone(),
        capability: args.capability.clone(),
        provider_id: binding
            .as_ref()
            .and_then(|binding| binding.get("providerId"))
            .and_then(Value::as_str)
            .unwrap_or("toolrouter.announced")
            .to_string(),
        adapter_id: binding
            .as_ref()
            .and_then(|binding| binding.get("adapterId"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| run.adapter_id.clone()),
        tool_name: tool_name.map(str::to_string),
        tool_namespace,
        tool_function,
        discovery_query,
        target_context: serde_json::json!({
            "packageRoot": package_root,
            "auditId": run.id,
            "workItemId": work_item.id,
            "capability": args.capability,
            "stage": work_item.stage,
        }),
        expected_verification_methods: string_array(schedule.get("verificationMethods")),
        reducer: CapabilityReducer {
            evidence_attestation: AuditEvidenceAttestation::RouterCaptured,
            accepted_evidence_source: "successful ToolRouter dispatch while this work item remains claimed".to_string(),
            note: "Generated code, model summaries, and knowledge citations do not satisfy this reducer. The returned tool must be called through the normal ToolRouter path so router evidence capture can persist normalized evidence.".to_string(),
        },
        instructions: instructions.to_string(),
    })
}

fn acquired_package_root(run: &AuditRun) -> Result<String, FunctionCallError> {
    run.metadata
        .get("acquiredTarget")
        .and_then(|target| target.get("root"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| model_error("audit run has no acquired target root"))
}

fn split_tool_name(tool_name: &str) -> (Option<String>, Option<String>) {
    if let Some((namespace, name)) = tool_name.rsplit_once("__")
        && !namespace.is_empty()
        && !name.is_empty()
    {
        return (Some(format!("{namespace}__")), Some(name.to_string()));
    }
    (None, Some(tool_name.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentAssignmentResponse {
    pub(super) assignment: AgentAssignmentSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArtifactResponse {
    pub(super) artifact_ref: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapabilityGapResponse {
    pub(super) artifact_ref: String,
    pub(super) work_item: WorkSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EvidenceResponse {
    pub(super) evidence_ref: String,
    pub(super) attestation: AuditEvidenceAttestation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinishWorkResponse {
    pub(super) work_item: WorkSummary,
    pub(super) agent_assignments: Vec<AgentAssignmentSummary>,
    pub(super) current_stage: AuditStageId,
    pub(super) remaining_pending: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalizeReportResponse {
    pub(super) run: RunSummary,
    pub(super) report_ref: String,
    pub(super) markdown_ref: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::{AuditProfile, AuditTarget};
    use serde_json::json;

    #[test]
    fn prepare_capability_uses_announced_tool_discovery_when_no_tool_is_bound() {
        let work_item_id = "audit-1:stage:0".to_string();
        let mut run_metadata = Metadata::new();
        run_metadata.insert(
            "acquiredTarget".to_string(),
            json!({
                "root": "/tmp/audit/input/package",
            }),
        );
        let mut work_metadata = Metadata::new();
        work_metadata.insert(
            "stageSchedule".to_string(),
            json!({
                "desiredCapabilities": ["static.analysis"],
                "availableCapabilities": [
                    {
                        "capability": "static.analysis",
                        "providerId": "peregrine.sui",
                        "adapterId": "peregrine.sui"
                    }
                ],
                "verificationMethods": ["staticAnalysis"]
            }),
        );
        let run = AuditRun {
            schema_version: 1,
            id: "audit-1".to_string(),
            plan_fingerprint: "plan".to_string(),
            target: AuditTarget::LocalPackage {
                chain_id: "sui".to_string(),
                path: "/tmp/source".to_string(),
                metadata: Metadata::new(),
            },
            profile: AuditProfile::default(),
            status: AuditRunStatus::Running,
            current_stage: AuditStageId::StaticAnalysis,
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: Some("peregrine.sui".to_string()),
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items: vec![AuditWorkItem {
                id: work_item_id.clone(),
                stage: AuditStageId::StaticAnalysis,
                status: AuditWorkItemStatus::Claimed,
                title: "Static analysis".to_string(),
                claimed_by: Some("coordinator".to_string()),
                attempts: 1,
                evidence_refs: Vec::new(),
                created_at: 1,
                updated_at: 1,
                metadata: work_metadata,
            }],
            agent_assignments: Vec::new(),
            evidence_refs: Vec::new(),
            artifact_refs: Vec::new(),
            created_at: 1,
            updated_at: 1,
            metadata: run_metadata,
        };

        let dispatch = prepare_capability_dispatch(
            &run,
            &PrepareCapabilityArgs {
                work_item_id,
                capability: "static.analysis".to_string(),
            },
        )
        .expect("dispatch");

        assert_eq!(dispatch.tool_name, None);
        assert_eq!(dispatch.tool_namespace, None);
        assert_eq!(dispatch.tool_function, None);
        assert_eq!(
            dispatch.discovery_query,
            Some("peregrine.sui static.analysis audit capability".to_string())
        );
        assert_eq!(
            dispatch.target_context.get("packageRoot"),
            Some(&json!("/tmp/audit/input/package"))
        );
        assert!(
            dispatch
                .instructions
                .contains("announced ToolRouter-visible tool")
        );
    }

    #[test]
    fn prepare_capability_discovers_desired_capability_without_binding() {
        let work_item_id = "audit-1:stage:0".to_string();
        let mut run_metadata = Metadata::new();
        run_metadata.insert(
            "acquiredTarget".to_string(),
            json!({
                "root": "/tmp/audit/input/package",
            }),
        );
        let mut work_metadata = Metadata::new();
        work_metadata.insert(
            "stageSchedule".to_string(),
            json!({
                "desiredCapabilities": ["bytecode.analysis"],
                "availableCapabilities": [],
                "unavailableCapabilities": [],
                "verificationMethods": ["bytecodeAnalysis"]
            }),
        );
        let run = AuditRun {
            schema_version: 1,
            id: "audit-1".to_string(),
            plan_fingerprint: "plan".to_string(),
            target: AuditTarget::LocalPackage {
                chain_id: "sui".to_string(),
                path: "/tmp/source".to_string(),
                metadata: Metadata::new(),
            },
            profile: AuditProfile::default(),
            status: AuditRunStatus::Running,
            current_stage: AuditStageId::BytecodeReview,
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: Some("peregrine.sui".to_string()),
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items: vec![AuditWorkItem {
                id: work_item_id.clone(),
                stage: AuditStageId::BytecodeReview,
                status: AuditWorkItemStatus::Claimed,
                title: "Bytecode review".to_string(),
                claimed_by: Some("coordinator".to_string()),
                attempts: 1,
                evidence_refs: Vec::new(),
                created_at: 1,
                updated_at: 1,
                metadata: work_metadata,
            }],
            agent_assignments: Vec::new(),
            evidence_refs: Vec::new(),
            artifact_refs: Vec::new(),
            created_at: 1,
            updated_at: 1,
            metadata: run_metadata,
        };

        let dispatch = prepare_capability_dispatch(
            &run,
            &PrepareCapabilityArgs {
                work_item_id,
                capability: "bytecode.analysis".to_string(),
            },
        )
        .expect("dispatch");

        assert_eq!(dispatch.provider_id, "toolrouter.announced");
        assert_eq!(
            dispatch.discovery_query,
            Some("peregrine.sui bytecode.analysis audit capability".to_string())
        );
        assert_eq!(dispatch.adapter_id, Some("peregrine.sui".to_string()));
        assert_eq!(dispatch.tool_name, None);
    }
}
