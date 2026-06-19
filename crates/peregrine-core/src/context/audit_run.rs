use super::ContextualUserFragment;
use peregrine_types::{
    AuditAgentAssignment, AuditAgentAssignmentStatus, AuditCoverageGap, AuditRun, AuditRunStatus,
    AuditStageId, AuditWorkItem, AuditWorkItemStatus,
};
use serde::Serialize;
use std::collections::BTreeMap;

const START_MARKER: &str = "<audit_run_context>";
const END_MARKER: &str = "</audit_run_context>";
const MAX_BODY_BYTES: usize = 6_000;
const MAX_REFS: usize = 12;
const MAX_WORK_ITEMS: usize = 8;
const MAX_AGENT_ASSIGNMENTS: usize = 8;
const MAX_GAPS: usize = 8;
const MAX_SCHEDULE_ITEMS: usize = 8;

/// Bounded model context describing the active persisted audit run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRunContextFragment {
    body: String,
}

impl AuditRunContextFragment {
    pub fn from_run(run: &AuditRun) -> Self {
        let context = AuditRunContext {
            audit_id: &run.id,
            plan_fingerprint: &run.plan_fingerprint,
            status: &run.status,
            current_stage: &run.current_stage,
            adapter_id: run.adapter_id.as_deref(),
            coordinator_thread_id: run.coordinator_thread_id.as_deref(),
            goal_id: run.goal_id.as_deref(),
            budget: AuditBudgetContext {
                model_token_budget: run.profile.model_token_budget,
                wall_time_seconds: run.profile.wall_time_seconds,
                max_hypotheses: run.profile.max_hypotheses,
            },
            counts: AuditRunCounts {
                coverage_gaps: run.coverage_gaps.len(),
                evidence_refs: run.evidence_refs.len(),
                artifact_refs: run.artifact_refs.len(),
                agent_assignments: agent_assignment_counts(run),
                work_items: work_item_counts(run),
            },
            current_work: current_work_items(run),
            agent_assignments: current_agent_assignments(run),
            coverage_gaps: run
                .coverage_gaps
                .iter()
                .take(MAX_GAPS)
                .map(CoverageGapContext::from)
                .collect(),
            evidence_refs: run
                .evidence_refs
                .iter()
                .take(MAX_REFS)
                .map(String::as_str)
                .collect(),
            artifact_refs: run
                .artifact_refs
                .iter()
                .take(MAX_REFS)
                .map(String::as_str)
                .collect(),
        };
        let mut body = serde_json::to_string_pretty(&context)
            .unwrap_or_else(|error| format!("{{\"serializationError\":\"{error}\"}}"));
        if body.len() > MAX_BODY_BYTES {
            body.truncate(MAX_BODY_BYTES);
        }
        Self { body }
    }
}

impl ContextualUserFragment for AuditRunContextFragment {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn body(&self) -> String {
        format!(
            "\nThis is bounded audit state. Retrieve details through registered audit tools.\n{}\n",
            self.body
        )
    }

    fn type_markers() -> (&'static str, &'static str) {
        (START_MARKER, END_MARKER)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditRunContext<'a> {
    audit_id: &'a str,
    plan_fingerprint: &'a str,
    status: &'a AuditRunStatus,
    current_stage: &'a AuditStageId,
    adapter_id: Option<&'a str>,
    coordinator_thread_id: Option<&'a str>,
    goal_id: Option<&'a str>,
    budget: AuditBudgetContext,
    counts: AuditRunCounts,
    current_work: Vec<WorkItemContext<'a>>,
    agent_assignments: Vec<AgentAssignmentContext<'a>>,
    coverage_gaps: Vec<CoverageGapContext<'a>>,
    evidence_refs: Vec<&'a str>,
    artifact_refs: Vec<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditBudgetContext {
    model_token_budget: i64,
    wall_time_seconds: i64,
    max_hypotheses: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditRunCounts {
    coverage_gaps: usize,
    evidence_refs: usize,
    artifact_refs: usize,
    agent_assignments: BTreeMap<&'static str, usize>,
    work_items: BTreeMap<&'static str, usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkItemContext<'a> {
    id: &'a str,
    stage: &'a AuditStageId,
    status: &'a AuditWorkItemStatus,
    title: &'a str,
    claimed_by: Option<&'a str>,
    attempts: u32,
    evidence_ref_count: usize,
    schedule: Option<StageScheduleContext>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StageScheduleContext {
    action: String,
    required_capabilities: Vec<String>,
    available_capabilities: Vec<String>,
    unavailable_capabilities: Vec<String>,
    verification_methods: Vec<String>,
}

impl<'a> From<&'a AuditWorkItem> for WorkItemContext<'a> {
    fn from(value: &'a AuditWorkItem) -> Self {
        Self {
            id: &value.id,
            stage: &value.stage,
            status: &value.status,
            title: &value.title,
            claimed_by: value.claimed_by.as_deref(),
            attempts: value.attempts,
            evidence_ref_count: value.evidence_refs.len(),
            schedule: stage_schedule_context(value),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentAssignmentContext<'a> {
    id: &'a str,
    work_item_id: &'a str,
    role: &'a str,
    role_name: &'a str,
    status: &'a AuditAgentAssignmentStatus,
    agent_thread_id: Option<&'a str>,
    conclusion_ref_count: usize,
}

impl<'a> From<&'a AuditAgentAssignment> for AgentAssignmentContext<'a> {
    fn from(value: &'a AuditAgentAssignment) -> Self {
        Self {
            id: &value.id,
            work_item_id: &value.work_item_id,
            role: match value.role {
                peregrine_types::AuditAgentRole::Researcher => "researcher",
                peregrine_types::AuditAgentRole::Skeptic => "skeptic",
                peregrine_types::AuditAgentRole::Exploiter => "exploiter",
                peregrine_types::AuditAgentRole::Judge => "judge",
            },
            role_name: &value.role_name,
            status: &value.status,
            agent_thread_id: value.agent_thread_id.as_deref(),
            conclusion_ref_count: value.conclusion_refs.len(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoverageGapContext<'a> {
    capability: &'a str,
    stage: &'a AuditStageId,
    reason: &'a str,
    required: bool,
}

impl<'a> From<&'a AuditCoverageGap> for CoverageGapContext<'a> {
    fn from(value: &'a AuditCoverageGap) -> Self {
        Self {
            capability: &value.capability,
            stage: &value.stage,
            reason: &value.reason,
            required: value.required,
        }
    }
}

fn current_work_items(run: &AuditRun) -> Vec<WorkItemContext<'_>> {
    let mut items = run
        .work_items
        .iter()
        .filter(|item| {
            item.stage == run.current_stage
                && matches!(
                    item.status,
                    AuditWorkItemStatus::Claimed | AuditWorkItemStatus::Pending
                )
        })
        .take(MAX_WORK_ITEMS)
        .map(WorkItemContext::from)
        .collect::<Vec<_>>();
    if !items.is_empty() {
        return items;
    }

    items = run
        .work_items
        .iter()
        .filter(|item| item.stage == run.current_stage)
        .take(MAX_WORK_ITEMS)
        .map(WorkItemContext::from)
        .collect::<Vec<_>>();
    if !items.is_empty() {
        return items;
    }

    run.work_items
        .iter()
        .filter(|item| item.status == AuditWorkItemStatus::Pending)
        .take(MAX_WORK_ITEMS)
        .map(WorkItemContext::from)
        .collect()
}

fn stage_schedule_context(work_item: &AuditWorkItem) -> Option<StageScheduleContext> {
    let schedule = work_item.metadata.get("stageSchedule")?;
    Some(StageScheduleContext {
        action: schedule.get("action")?.as_str()?.to_string(),
        required_capabilities: string_array(schedule.get("requiredCapabilities")),
        available_capabilities: capability_array(schedule.get("availableCapabilities")),
        unavailable_capabilities: unavailable_capability_array(
            schedule.get("unavailableCapabilities"),
        ),
        verification_methods: string_array(schedule.get("verificationMethods")),
    })
}

fn string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .take(MAX_SCHEDULE_ITEMS)
        .map(str::to_string)
        .collect()
}

fn capability_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.get("capability").and_then(serde_json::Value::as_str))
        .take(MAX_SCHEDULE_ITEMS)
        .map(str::to_string)
        .collect()
}

fn unavailable_capability_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let capability = value.get("capability")?.as_str()?;
            let reason = value.get("reason")?.as_str()?;
            Some(format!("{capability}: {reason}"))
        })
        .take(MAX_SCHEDULE_ITEMS)
        .collect()
}

fn current_agent_assignments(run: &AuditRun) -> Vec<AgentAssignmentContext<'_>> {
    let current_work_ids = run
        .work_items
        .iter()
        .filter(|item| {
            item.stage == run.current_stage
                && matches!(
                    item.status,
                    AuditWorkItemStatus::Claimed | AuditWorkItemStatus::Pending
                )
        })
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    let mut assignments = run
        .agent_assignments
        .iter()
        .filter(|assignment| {
            current_work_ids
                .iter()
                .any(|work_item_id| *work_item_id == assignment.work_item_id)
        })
        .take(MAX_AGENT_ASSIGNMENTS)
        .map(AgentAssignmentContext::from)
        .collect::<Vec<_>>();
    if !assignments.is_empty() {
        return assignments;
    }

    assignments = run
        .agent_assignments
        .iter()
        .filter(|assignment| assignment.status == AuditAgentAssignmentStatus::Pending)
        .take(MAX_AGENT_ASSIGNMENTS)
        .map(AgentAssignmentContext::from)
        .collect::<Vec<_>>();
    if !assignments.is_empty() {
        return assignments;
    }

    run.agent_assignments
        .iter()
        .take(MAX_AGENT_ASSIGNMENTS)
        .map(AgentAssignmentContext::from)
        .collect()
}

fn agent_assignment_counts(run: &AuditRun) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for assignment in &run.agent_assignments {
        let status = match assignment.status {
            AuditAgentAssignmentStatus::Pending => "pending",
            AuditAgentAssignmentStatus::Spawned => "spawned",
            AuditAgentAssignmentStatus::Completed => "completed",
            AuditAgentAssignmentStatus::Failed => "failed",
            AuditAgentAssignmentStatus::Cancelled => "cancelled",
        };
        *counts.entry(status).or_default() += 1;
    }
    counts
}

fn work_item_counts(run: &AuditRun) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for item in &run.work_items {
        let status = match item.status {
            AuditWorkItemStatus::Pending => "pending",
            AuditWorkItemStatus::Claimed => "claimed",
            AuditWorkItemStatus::Completed => "completed",
            AuditWorkItemStatus::Failed => "failed",
            AuditWorkItemStatus::Blocked => "blocked",
            AuditWorkItemStatus::Cancelled => "cancelled",
        };
        *counts.entry(status).or_default() += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::{
        AuditAgentAssignment, AuditAgentAssignmentStatus, AuditAgentRole, AuditCoverageGap,
        AuditProfile, AuditRunStatus, AuditStageId, AuditTarget, AuditWorkItem,
        AuditWorkItemStatus, Metadata,
    };

    #[test]
    fn fragment_is_bounded() {
        let run = AuditRun {
            schema_version: 1,
            id: "audit-1".to_string(),
            plan_fingerprint: "fingerprint".to_string(),
            target: AuditTarget::LocalPackage {
                chain_id: "sui".to_string(),
                path: "/tmp/package".to_string(),
                metadata: Metadata::new(),
            },
            profile: AuditProfile::default(),
            status: AuditRunStatus::Running,
            current_stage: AuditStageId::BuildNormalize,
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: None,
            capabilities: Vec::new(),
            coverage_gaps: (0..100)
                .map(|index| AuditCoverageGap {
                    capability: format!("capability-{index}"),
                    stage: AuditStageId::BuildNormalize,
                    reason: format!("reason-{index}"),
                    required: true,
                })
                .collect(),
            work_items: (0..100)
                .map(|index| {
                    let mut metadata = Metadata::new();
                    if index == 0 {
                        metadata.insert(
                            "stageSchedule".to_string(),
                            serde_json::json!({
                                "action": "useAvailableCapabilities",
                                "requiredCapabilities": ["target.acquire"],
                                "availableCapabilities": [
                                    {"capability": "target.acquire"}
                                ],
                                "unavailableCapabilities": [],
                                "verificationMethods": [],
                            }),
                        );
                    }
                    AuditWorkItem {
                        id: format!("work-{index}"),
                        stage: if index == 0 {
                            AuditStageId::BuildNormalize
                        } else {
                            AuditStageId::AttackSurface
                        },
                        status: if index == 0 {
                            AuditWorkItemStatus::Claimed
                        } else {
                            AuditWorkItemStatus::Pending
                        },
                        title: format!("Work item {index}"),
                        claimed_by: (index == 0).then_some("researcher".to_string()),
                        attempts: index,
                        evidence_refs: Vec::new(),
                        created_at: 0,
                        updated_at: 0,
                        metadata,
                    }
                })
                .collect(),
            agent_assignments: (0..100)
                .map(|index| AuditAgentAssignment {
                    schema_version: 1,
                    id: format!("assignment-{index}"),
                    audit_run_id: "audit-1".to_string(),
                    work_item_id: if index == 0 {
                        "work-0".to_string()
                    } else {
                        format!("work-{index}")
                    },
                    role: AuditAgentRole::Researcher,
                    role_name: "audit-researcher".to_string(),
                    status: AuditAgentAssignmentStatus::Pending,
                    agent_thread_id: None,
                    conclusion_refs: Vec::new(),
                    created_at: 0,
                    updated_at: 0,
                    metadata: Metadata::new(),
                })
                .collect(),
            evidence_refs: (0..100).map(|index| format!("evidence-{index}")).collect(),
            artifact_refs: (0..100).map(|index| format!("artifact-{index}")).collect(),
            created_at: 0,
            updated_at: 0,
            metadata: Metadata::new(),
        };

        let rendered = AuditRunContextFragment::from_run(&run).render();

        assert!(rendered.len() <= MAX_BODY_BYTES + 300);
        assert!(rendered.contains("audit-1"));
        assert!(rendered.contains("\"status\": \"running\""));
        assert!(rendered.contains("\"currentStage\": \"buildNormalize\""));
        assert!(rendered.contains("\"currentWork\""));
        assert!(rendered.contains("work-0"));
        assert!(rendered.contains("\"schedule\""));
        assert!(rendered.contains("target.acquire"));
        assert!(rendered.contains("\"agentAssignments\""));
        assert!(rendered.contains("assignment-0"));
        assert!(rendered.contains("\"evidenceRefs\": 100"));
        assert!(!rendered.contains("evidence-99"));
        assert!(!rendered.contains("assignment-99"));
        assert!(!rendered.contains("capability-99"));
    }
}
