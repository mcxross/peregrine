mod scheduler;

pub use scheduler::{
    AuditStageAvailableCapability, AuditStageSchedule, AuditStageScheduleAction,
    AuditStageUnavailableCapability, STAGE_SCHEDULE_METADATA_KEY, attach_stage_schedules,
    schedule_metadata, stage_desired_capabilities, stage_schedule,
};

use peregrine_types::{
    AuditAgentAssignment, AuditAgentAssignmentStatus, AuditAgentRole, AuditCapabilityBinding,
    AuditProfile, AuditStageId, AuditTarget, AuditWorkItem, AuditWorkItemStatus, ExploitBundle,
    ExploitIntent, Metadata, ToolDiagnostic,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    future::Future,
    path::{Component, Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use thiserror::Error;

pub type AdapterFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, AuditAdapterError>> + Send + 'a>>;

pub fn default_audit_stages() -> Vec<AuditStageId> {
    vec![
        AuditStageId::BuildNormalize,
        AuditStageId::SemanticGraphs,
        AuditStageId::BytecodeReview,
        AuditStageId::AttackSurface,
        AuditStageId::Invariants,
        AuditStageId::AttackHypotheses,
        AuditStageId::VerificationPlanning,
        AuditStageId::TargetedTests,
        AuditStageId::DynamicAnalysis,
        AuditStageId::SymbolicExecution,
        AuditStageId::EconomicSimulation,
        AuditStageId::AdversarialReview,
        AuditStageId::EvidenceAggregation,
        AuditStageId::FindingValidation,
        AuditStageId::SeverityRanking,
        AuditStageId::AuditReport,
        AuditStageId::AuditTrace,
    ]
}

pub fn create_audit_work_items(
    audit_id: &str,
    stages: &[AuditStageId],
    created_at: i64,
) -> Vec<AuditWorkItem> {
    stages
        .iter()
        .enumerate()
        .map(|(index, stage)| AuditWorkItem {
            id: format!("{audit_id}:stage:{index}"),
            stage: stage.clone(),
            status: AuditWorkItemStatus::Pending,
            title: format!("Complete {stage:?} stage"),
            claimed_by: None,
            attempts: 0,
            evidence_refs: Vec::new(),
            created_at,
            updated_at: created_at,
            metadata: Metadata::new(),
        })
        .collect()
}

pub fn create_audit_agent_assignments(
    audit_id: &str,
    work_items: &[AuditWorkItem],
    created_at: i64,
) -> Vec<AuditAgentAssignment> {
    work_items
        .iter()
        .flat_map(|work_item| {
            default_agent_roles_for_stage(&work_item.stage)
                .into_iter()
                .map(move |(role, role_name)| (work_item, role, role_name))
        })
        .enumerate()
        .map(
            |(index, (work_item, role, role_name))| AuditAgentAssignment {
                schema_version: 1,
                id: format!("{audit_id}:agent:{index}"),
                audit_run_id: audit_id.to_string(),
                work_item_id: work_item.id.clone(),
                role,
                role_name: role_name.to_string(),
                status: AuditAgentAssignmentStatus::Pending,
                agent_thread_id: None,
                conclusion_refs: Vec::new(),
                created_at,
                updated_at: created_at,
                metadata: Metadata::new(),
            },
        )
        .collect()
}

fn default_agent_roles_for_stage(stage: &AuditStageId) -> Vec<(AuditAgentRole, &'static str)> {
    match stage {
        AuditStageId::AttackSurface | AuditStageId::Invariants | AuditStageId::AttackHypotheses => {
            vec![(AuditAgentRole::Researcher, "audit-researcher")]
        }
        AuditStageId::TargetedTests
        | AuditStageId::DynamicAnalysis
        | AuditStageId::ExploitConfirmation => {
            vec![(AuditAgentRole::Exploiter, "audit-exploiter")]
        }
        AuditStageId::AdversarialReview => vec![
            (AuditAgentRole::Researcher, "audit-researcher"),
            (AuditAgentRole::Skeptic, "audit-skeptic"),
            (AuditAgentRole::Exploiter, "audit-exploiter"),
            (AuditAgentRole::Judge, "audit-judge"),
        ],
        AuditStageId::FindingValidation => vec![
            (AuditAgentRole::Skeptic, "audit-skeptic"),
            (AuditAgentRole::Judge, "audit-judge"),
        ],
        AuditStageId::SeverityRanking | AuditStageId::AuditReport => {
            vec![(AuditAgentRole::Judge, "audit-judge")]
        }
        AuditStageId::AuditSession
        | AuditStageId::BuildNormalize
        | AuditStageId::SemanticGraphs
        | AuditStageId::Classification
        | AuditStageId::ThreatModel
        | AuditStageId::FunctionRiskMap
        | AuditStageId::StaticAnalysis
        | AuditStageId::GraphAnalysis
        | AuditStageId::BytecodeReview
        | AuditStageId::VerificationPlanning
        | AuditStageId::InvariantStress
        | AuditStageId::SymbolicExecution
        | AuditStageId::EconomicSimulation
        | AuditStageId::EvidenceAggregation
        | AuditStageId::Remediation
        | AuditStageId::RegressionTests
        | AuditStageId::AuditTrace
        | AuditStageId::FixVerification => Vec::new(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditWorkspace {
    pub root: PathBuf,
    pub input: PathBuf,
    pub workspace: PathBuf,
    pub artifacts: PathBuf,
    pub evidence: PathBuf,
    pub traces: PathBuf,
    pub reports: PathBuf,
}

impl AuditWorkspace {
    pub fn create(audits_root: &Path, audit_id: &str) -> Result<Self, AuditAdapterError> {
        let mut components = Path::new(audit_id).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(AuditAdapterError::InvalidTarget(
                "audit ID must be one path component".to_string(),
            ));
        }
        let root = audits_root.join(audit_id);
        let value = Self {
            input: root.join("input"),
            workspace: root.join("workspace"),
            artifacts: root.join("artifacts"),
            evidence: root.join("evidence"),
            traces: root.join("traces"),
            reports: root.join("reports"),
            root,
        };
        for directory in [
            &value.input,
            &value.workspace,
            &value.artifacts,
            &value.evidence,
            &value.traces,
            &value.reports,
        ] {
            std::fs::create_dir_all(directory).map_err(|source| AuditAdapterError::Io {
                action: "create audit workspace",
                source,
            })?;
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditTargetPreflight {
    pub adapter_id: String,
    pub normalized_target: AuditTarget,
    pub capabilities: Vec<AuditCapabilityBinding>,
    pub diagnostics: Vec<ToolDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcquiredAuditTarget {
    pub adapter_id: String,
    pub root: PathBuf,
    pub manifest_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<String>,
    pub immutable_state_ref: Option<String>,
    pub diagnostics: Vec<ToolDiagnostic>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploitReplay {
    pub bundle_id: String,
    pub succeeded: bool,
    pub evidence_refs: Vec<String>,
    pub diagnostics: Vec<ToolDiagnostic>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Blockchain adapter used by the neutral audit coordinator.
///
/// Implementations own chain-specific target formats, imports, exploit encoding,
/// replay, and state hydration. They must only write beneath the supplied audit
/// workspace.
pub trait AuditChainAdapter: Send + Sync {
    fn adapter_id(&self) -> &'static str;
    fn chain_id(&self) -> &'static str;
    fn capabilities(&self) -> Vec<AuditCapabilityBinding>;
    fn preflight<'a>(&'a self, target: &'a AuditTarget) -> AdapterFuture<'a, AuditTargetPreflight>;
    fn acquire<'a>(
        &'a self,
        target: &'a AuditTarget,
        profile: &'a AuditProfile,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, AcquiredAuditTarget>;
    fn encode_exploit<'a>(
        &'a self,
        target: &'a AcquiredAuditTarget,
        intent: &'a ExploitIntent,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitBundle>;
    fn replay_exploit<'a>(
        &'a self,
        target: &'a AcquiredAuditTarget,
        bundle: &'a ExploitBundle,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitReplay>;
}

#[derive(Default)]
pub struct AuditAdapterRegistry {
    adapters: BTreeMap<String, Arc<dyn AuditChainAdapter>>,
}

impl AuditAdapterRegistry {
    pub fn register(&mut self, adapter: Arc<dyn AuditChainAdapter>) {
        self.adapters
            .insert(adapter.chain_id().to_string(), adapter);
    }

    pub fn get(&self, chain_id: &str) -> Result<Arc<dyn AuditChainAdapter>, AuditAdapterError> {
        self.adapters
            .get(chain_id)
            .cloned()
            .ok_or_else(|| AuditAdapterError::UnsupportedChain(chain_id.to_string()))
    }

    pub fn chain_ids(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
}

#[derive(Debug, Error)]
pub enum AuditAdapterError {
    #[error("unsupported audit chain `{0}`")]
    UnsupportedChain(String),
    #[error("invalid audit target: {0}")]
    InvalidTarget(String),
    #[error("audit adapter failed: {0}")]
    Adapter(String),
    #[error("io error while {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_rejects_path_like_audit_ids() {
        let root = tempfile::tempdir().expect("tempdir");

        let error = AuditWorkspace::create(root.path(), "../outside").expect_err("invalid ID");

        assert!(matches!(error, AuditAdapterError::InvalidTarget(_)));
    }

    #[test]
    fn creates_default_agent_assignments_for_adversarial_stages() {
        let work_items = create_audit_work_items(
            "audit-1",
            &[
                AuditStageId::BuildNormalize,
                AuditStageId::AttackHypotheses,
                AuditStageId::AdversarialReview,
                AuditStageId::FindingValidation,
                AuditStageId::AuditReport,
            ],
            10,
        );

        let assignments = create_audit_agent_assignments("audit-1", &work_items, 10);
        let summary = assignments
            .iter()
            .map(|assignment| {
                (
                    assignment.work_item_id.as_str(),
                    &assignment.role,
                    assignment.role_name.as_str(),
                    &assignment.status,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            summary,
            vec![
                (
                    "audit-1:stage:1",
                    &AuditAgentRole::Researcher,
                    "audit-researcher",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:2",
                    &AuditAgentRole::Researcher,
                    "audit-researcher",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:2",
                    &AuditAgentRole::Skeptic,
                    "audit-skeptic",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:2",
                    &AuditAgentRole::Exploiter,
                    "audit-exploiter",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:2",
                    &AuditAgentRole::Judge,
                    "audit-judge",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:3",
                    &AuditAgentRole::Skeptic,
                    "audit-skeptic",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:3",
                    &AuditAgentRole::Judge,
                    "audit-judge",
                    &AuditAgentAssignmentStatus::Pending,
                ),
                (
                    "audit-1:stage:4",
                    &AuditAgentRole::Judge,
                    "audit-judge",
                    &AuditAgentAssignmentStatus::Pending,
                ),
            ]
        );
    }

    #[test]
    fn attaches_stage_schedules_from_capability_bindings() {
        let mut work_items = create_audit_work_items(
            "audit-1",
            &[
                AuditStageId::BuildNormalize,
                AuditStageId::AttackSurface,
                AuditStageId::DynamicAnalysis,
                AuditStageId::AuditReport,
            ],
            10,
        );
        let capabilities = vec![
            AuditCapabilityBinding {
                capability: "target.acquire".to_string(),
                provider_id: "adapter".to_string(),
                adapter_id: Some("test-adapter".to_string()),
                tool_name: None,
                available: true,
                diagnostic: None,
            },
            AuditCapabilityBinding {
                capability: "target.normalize".to_string(),
                provider_id: "adapter".to_string(),
                adapter_id: Some("test-adapter".to_string()),
                tool_name: None,
                available: true,
                diagnostic: None,
            },
            AuditCapabilityBinding {
                capability: "static.analysis".to_string(),
                provider_id: "mcp".to_string(),
                adapter_id: None,
                tool_name: None,
                available: true,
                diagnostic: None,
            },
            AuditCapabilityBinding {
                capability: "graph.analysis".to_string(),
                provider_id: "mcp".to_string(),
                adapter_id: None,
                tool_name: None,
                available: false,
                diagnostic: Some("graph unavailable".to_string()),
            },
            AuditCapabilityBinding {
                capability: "dynamic.fuzzing".to_string(),
                provider_id: "mcp".to_string(),
                adapter_id: None,
                tool_name: Some("mcp__audit__fuzz".to_string()),
                available: false,
                diagnostic: Some("fuzzer unavailable".to_string()),
            },
        ];

        attach_stage_schedules(&mut work_items, &capabilities).expect("attach schedules");

        let build_schedule = schedule_metadata(&work_items[0].metadata).expect("build schedule");
        assert_eq!(
            build_schedule.action,
            AuditStageScheduleAction::UseAvailableCapabilities
        );
        assert_eq!(
            build_schedule
                .available_capabilities
                .iter()
                .map(|capability| capability.capability.as_str())
                .collect::<Vec<_>>(),
            vec!["target.acquire", "target.normalize"]
        );

        let partial_schedule =
            schedule_metadata(&work_items[1].metadata).expect("partial schedule");
        assert_eq!(
            partial_schedule.action,
            AuditStageScheduleAction::UseAvailableCapabilitiesWithGaps
        );
        assert_eq!(
            partial_schedule
                .available_capabilities
                .iter()
                .map(|capability| capability.capability.as_str())
                .collect::<Vec<_>>(),
            vec!["static.analysis"]
        );
        assert_eq!(
            partial_schedule.unavailable_capabilities,
            vec![AuditStageUnavailableCapability {
                capability: "graph.analysis".to_string(),
                reason: "graph unavailable".to_string(),
            }]
        );

        let fuzz_schedule = schedule_metadata(&work_items[2].metadata).expect("fuzz schedule");
        assert_eq!(
            fuzz_schedule.action,
            AuditStageScheduleAction::RecordUnavailableAndContinue
        );
        assert_eq!(
            fuzz_schedule.unavailable_capabilities,
            vec![AuditStageUnavailableCapability {
                capability: "dynamic.fuzzing".to_string(),
                reason: "fuzzer unavailable".to_string(),
            }]
        );

        let report_schedule = schedule_metadata(&work_items[3].metadata).expect("report schedule");
        assert_eq!(report_schedule.action, AuditStageScheduleAction::ModelOnly);
        assert!(report_schedule.desired_capabilities.is_empty());
    }

    #[test]
    fn unregistered_desired_capabilities_are_discovered_at_runtime() {
        let schedule = stage_schedule(&AuditStageId::BytecodeReview, &[]);

        assert_eq!(
            schedule.action,
            AuditStageScheduleAction::UseAvailableCapabilities
        );
        assert_eq!(schedule.desired_capabilities, vec!["bytecode.analysis"]);
        assert!(schedule.available_capabilities.is_empty());
        assert!(schedule.unavailable_capabilities.is_empty());
    }
}
