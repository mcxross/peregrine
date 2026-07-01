#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type ToolInput = Value;
pub type JsonSchema = Value;
pub type Metadata = BTreeMap<String, Value>;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ToolRunStatus {
    Succeeded,
    Failed,
    Denied,
    Skipped,
    TimedOut,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ToolActionClass {
    ReadOnly,
    ToolExecution,
    GeneratedFileWrite,
    SourceCodeModification,
    DependencyModification,
    PackagePublishing,
    NetworkAccess,
    SecretAccess,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceConfidence {
    Confirmed,
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourcePrecision {
    Compiler,
    SourceMap,
    Bytecode,
    Summary,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FindingCandidateStatus {
    Confirmed,
    Likely,
    Possible,
    Hypothesis,
    FalsePositive,
    Informational,
    NeedsHumanReview,
    NeedsValidation,
    Fixed,
    Accepted,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FindingCandidateSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
    ToolOutput,
    ToolFailure,
    CodeLocation,
    TestResult,
    FuzzCounterexample,
    ProverResult,
    DependencyDiff,
    HumanApproval,
    AcceptedRisk,
    AgentOutput,
    Diagnostic,
    GraphSignal,
    BytecodeSignal,
    StaticFinding,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeLocation {
    pub file: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolPrerequisite {
    pub tool_id: String,
    pub reason: String,
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCost {
    pub risk: ToolRiskLevel,
    pub expected_latency_ms: Option<u64>,
    pub token_budget_hint: Option<usize>,
    pub output_budget_tokens: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolSideEffect {
    pub action_class: ToolActionClass,
    pub description: String,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolManifest {
    pub id: String,
    pub version: String,
    pub chain: Option<String>,
    pub category: String,
    pub description: String,
    pub when_to_use: Vec<String>,
    pub when_not_to_use: Vec<String>,
    pub prerequisites: Vec<ToolPrerequisite>,
    pub input_schema: JsonSchema,
    pub output_schema: Option<JsonSchema>,
    pub cost: ToolCost,
    pub action_class: ToolActionClass,
    pub side_effects: Vec<ToolSideEffect>,
    pub timeout_ms: Option<u64>,
    pub reducer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRunContext {
    pub session_id: Option<String>,
    pub task_id: String,
    pub target: Option<String>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRunArtifact {
    pub id: String,
    pub kind: String,
    pub path: Option<String>,
    pub content_hash: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolMetric {
    pub name: String,
    pub value: Value,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDiagnostic {
    pub level: String,
    pub source: String,
    pub message: String,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub id: String,
    pub kind: EvidenceKind,
    pub claim: String,
    pub observation: String,
    pub confidence: EvidenceConfidence,
    pub source_precision: SourcePrecision,
    pub location: Option<CodeLocation>,
    pub symbol_refs: Vec<String>,
    pub tool_run_id: Option<String>,
    pub raw_ref: Option<String>,
    pub freshness: Option<String>,
    pub follow_up: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPlan {
    pub commands: Vec<String>,
    pub expected_evidence: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PatchRecommendation {
    pub summary: String,
    pub affected_locations: Vec<CodeLocation>,
    pub minimal_change: String,
    pub regression_tests: Vec<String>,
    pub verification_commands: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FindingCandidate {
    pub id: String,
    pub title: String,
    pub category: String,
    pub severity: FindingCandidateSeverity,
    pub confidence: EvidenceConfidence,
    pub status: FindingCandidateStatus,
    pub affected_symbols: Vec<String>,
    pub exploit_scenario: Option<String>,
    pub evidence_refs: Vec<String>,
    pub validation_plan: ValidationPlan,
    pub patch_recommendation: Option<PatchRecommendation>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRunResult {
    pub run_id: String,
    pub tool_id: String,
    pub status: ToolRunStatus,
    pub started_at: String,
    pub duration_ms: u64,
    pub target: Option<String>,
    pub input_hash: String,
    pub summary: String,
    pub evidence: Vec<EvidenceItem>,
    pub findings: Vec<FindingCandidate>,
    pub metrics: Vec<ToolMetric>,
    pub diagnostics: Vec<ToolDiagnostic>,
    pub artifacts: Vec<ToolRunArtifact>,
    pub raw_ref: Option<String>,
}

pub trait SecurityTool: Send + Sync {
    fn manifest(&self) -> ToolManifest;

    fn run(&self, input: ToolInput, context: ToolRunContext) -> ToolRunResult;
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditStageId {
    AuditSession,
    BuildNormalize,
    SemanticGraphs,
    Classification,
    ThreatModel,
    AttackSurface,
    FunctionRiskMap,
    Invariants,
    StaticAnalysis,
    GraphAnalysis,
    BytecodeReview,
    AttackHypotheses,
    VerificationPlanning,
    TargetedTests,
    DynamicAnalysis,
    InvariantStress,
    SymbolicExecution,
    EconomicSimulation,
    ExploitConfirmation,
    AdversarialReview,
    EvidenceAggregation,
    FindingValidation,
    SeverityRanking,
    Remediation,
    RegressionTests,
    AuditReport,
    AuditTrace,
    FixVerification,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditStageStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Blocked,
    Unavailable,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AuditTarget {
    LocalPackage {
        chain_id: String,
        path: String,
        #[serde(default, skip_serializing_if = "Metadata::is_empty")]
        metadata: Metadata,
    },
    RemotePackage {
        chain_id: String,
        network_id: String,
        package_ref: String,
        source_uri: Option<String>,
        state_ref: Option<String>,
        #[serde(default, skip_serializing_if = "Metadata::is_empty")]
        metadata: Metadata,
    },
}

impl AuditTarget {
    pub fn chain_id(&self) -> &str {
        match self {
            Self::LocalPackage { chain_id, .. } | Self::RemotePackage { chain_id, .. } => chain_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditProfile {
    pub model_token_budget: i64,
    pub wall_time_seconds: i64,
    pub max_hypotheses: u32,
    pub max_dependency_depth: u32,
    pub max_dependency_packages: u32,
}

impl Default for AuditProfile {
    fn default() -> Self {
        Self {
            model_token_budget: 500_000,
            wall_time_seconds: 4 * 60 * 60,
            max_hypotheses: 500,
            max_dependency_depth: 3,
            max_dependency_packages: 64,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditRunStatus {
    Pending,
    Running,
    Paused,
    Completed,
    CompletedWithGaps,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditWorkItemStatus {
    Pending,
    Claimed,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VerificationMethod {
    StaticAnalysis,
    GraphAnalysis,
    BytecodeAnalysis,
    GeneratedTest,
    Fuzzing,
    SymbolicExecution,
    FormalVerification,
    EconomicSimulation,
    ExploitReplay,
    HumanReview,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditAgentRole {
    Researcher,
    Skeptic,
    Exploiter,
    Judge,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditAgentConclusionStatus {
    Candidate,
    Supported,
    Refuted,
    NeedsValidation,
    Discarded,
    Accepted,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditAgentAssignmentStatus {
    Pending,
    Spawned,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditAgentAssignment {
    pub schema_version: u8,
    pub id: String,
    pub audit_run_id: String,
    pub work_item_id: String,
    pub role: AuditAgentRole,
    pub role_name: String,
    pub status: AuditAgentAssignmentStatus,
    pub agent_thread_id: Option<String>,
    pub conclusion_refs: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditAgentConclusion {
    pub schema_version: u8,
    pub id: String,
    pub audit_run_id: String,
    pub work_item_id: String,
    pub role: AuditAgentRole,
    pub agent_thread_id: Option<String>,
    pub status: AuditAgentConclusionStatus,
    pub summary: String,
    pub candidate_ids: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub artifact_refs: Vec<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditEvidenceAttestation {
    ModelSubmitted,
    RouterCaptured,
    AdapterReplay,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvidence {
    pub id: String,
    pub audit_run_id: String,
    pub work_item_id: Option<String>,
    pub verification_method: VerificationMethod,
    pub provider_id: String,
    pub adapter_id: Option<String>,
    pub tool_name: String,
    pub tool_version: Option<String>,
    pub input_hash: String,
    pub source_precision: SourcePrecision,
    pub attestation: AuditEvidenceAttestation,
    pub summary: String,
    pub observation: String,
    pub execution_trace_ref: Option<String>,
    pub artifact_refs: Vec<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditCapabilityBinding {
    pub capability: String,
    pub provider_id: String,
    pub adapter_id: Option<String>,
    pub tool_name: Option<String>,
    pub available: bool,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditCoverageGap {
    pub capability: String,
    pub stage: AuditStageId,
    pub reason: String,
    pub affects_terminal_status: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditWorkItem {
    pub id: String,
    pub stage: AuditStageId,
    pub status: AuditWorkItemStatus,
    pub title: String,
    pub claimed_by: Option<String>,
    pub attempts: u32,
    pub evidence_refs: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploitIntent {
    pub id: String,
    pub hypothesis_id: String,
    pub summary: String,
    pub entrypoints: Vec<String>,
    pub expected_violation: String,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub parameters: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploitBundle {
    pub id: String,
    pub adapter_id: String,
    pub intent_id: String,
    pub format: String,
    pub artifact_refs: Vec<String>,
    pub replayable: bool,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditPlannerOutput {
    pub summary: String,
    pub rationale: String,
    pub focus_areas: Vec<String>,
    pub non_goals: Vec<String>,
    pub stage_plans: Vec<AuditStagePlan>,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditStagePlan {
    pub stage: AuditStageId,
    pub objective: String,
    pub rationale: String,
    pub focus: Vec<String>,
    pub desired_capabilities: Vec<String>,
    pub agent_roles: Vec<AuditAgentRole>,
    pub success_criteria: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditPlan {
    pub schema_version: u8,
    pub id: String,
    pub fingerprint: String,
    pub target: AuditTarget,
    pub profile: AuditProfile,
    pub stages: Vec<AuditStageId>,
    pub desired_capabilities: Vec<String>,
    pub planner_output: AuditPlannerOutput,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditRun {
    pub schema_version: u8,
    pub id: String,
    pub plan_fingerprint: String,
    pub target: AuditTarget,
    pub profile: AuditProfile,
    pub status: AuditRunStatus,
    pub current_stage: AuditStageId,
    pub coordinator_thread_id: Option<String>,
    pub goal_id: Option<String>,
    pub adapter_id: Option<String>,
    pub capabilities: Vec<AuditCapabilityBinding>,
    pub coverage_gaps: Vec<AuditCoverageGap>,
    pub work_items: Vec<AuditWorkItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_assignments: Vec<AuditAgentAssignment>,
    pub evidence_refs: Vec<String>,
    pub artifact_refs: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub schema_version: u8,
    pub audit_run_id: String,
    pub status: AuditRunStatus,
    pub findings: Vec<FindingCandidate>,
    pub coverage_gaps: Vec<AuditCoverageGap>,
    pub evidence_refs: Vec<String>,
    pub generated_at: i64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditFindingState {
    Confirmed,
    Likely,
    Possible,
    FalsePositive,
    Informational,
    NeedsHumanReview,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditFixState {
    Open,
    Fixed,
    PartiallyFixed,
    RegressionAdded,
    RiskAccepted,
    FalsePositive,
    NeedsReview,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditStageRun {
    pub id: String,
    pub stage_id: AuditStageId,
    pub status: AuditStageStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub summary: String,
    pub artifact_name: Option<String>,
    pub filename: Option<String>,
    pub evidence_ref: Option<EvidenceRef>,
    pub diagnostics: Vec<ToolDiagnostic>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub id: String,
    pub kind: EvidenceKind,
    pub summary: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditSessionPacket {
    pub schema_version: u8,
    pub id: String,
    pub project: String,
    pub repo_root: String,
    pub commit: String,
    pub package_manifest: String,
    pub target_modules: Vec<String>,
    pub compiler_version: Option<String>,
    pub dependency_graph: Option<Value>,
    pub selected_chain_adapter: String,
    pub enabled_tools: Vec<String>,
    pub audit_profile: String,
    pub threat_model_profile: String,
    pub timestamp: String,
    pub tool_versions: BTreeMap<String, String>,
    pub policy_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditPacket {
    pub schema_version: u8,
    pub audit_session_id: String,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub body: Metadata,
}

pub type CanonicalProjectIndexPacket = AuditPacket;
pub type AuditKnowledgeGraphPacket = AuditPacket;
pub type ContractClassificationPacket = AuditPacket;
pub type ThreatModelPacket = AuditPacket;
pub type FunctionRiskMapPacket = AuditPacket;
pub type InvariantRegistryPacket = AuditPacket;
pub type StaticFindingsPacket = AuditPacket;
pub type GraphEvidencePacket = AuditPacket;
pub type BytecodeReviewPacket = AuditPacket;
pub type AttackHypothesisPacket = AuditPacket;
pub type TestPlanPacket = AuditPacket;
pub type DynamicEvidencePacket = AuditPacket;
pub type InvariantStressPacket = AuditPacket;
pub type ConfirmedFindingsPacket = AuditPacket;
pub type SeverityRankingPacket = AuditPacket;
pub type RemediationPlanPacket = AuditPacket;
pub type RegressionTestPacket = AuditPacket;
pub type AuditReportPacket = AuditPacket;
pub type FixVerificationPacket = AuditPacket;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditTraceArtifact {
    pub name: String,
    pub filename: String,
    pub evidence_ref: Option<EvidenceRef>,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditTrace {
    pub schema_version: u8,
    pub audit_session_id: String,
    pub generated_at: String,
    pub artifacts: Vec<AuditTraceArtifact>,
    pub stage_runs: Vec<AuditStageRun>,
    pub findings: Vec<FindingCandidate>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_plan_enum_names() {
        assert_eq!(json!(ToolRunStatus::TimedOut), json!("timedOut"));
        assert_eq!(
            json!(FindingCandidateStatus::FalsePositive),
            json!("falsePositive")
        );
        assert_eq!(
            json!(FindingCandidateStatus::NeedsHumanReview),
            json!("needsHumanReview")
        );
        assert_eq!(
            json!(ToolActionClass::ToolExecution),
            json!("toolExecution")
        );
        assert_eq!(
            json!(AuditFixState::PartiallyFixed),
            json!("partiallyFixed")
        );
        assert_eq!(
            json!(AuditTarget::RemotePackage {
                chain_id: "sui".to_string(),
                network_id: "mainnet".to_string(),
                package_ref: "0x1".to_string(),
                source_uri: Some("https://example.invalid/graphql".to_string()),
                state_ref: Some("123".to_string()),
                metadata: Metadata::new(),
            }),
            json!({
                "type": "remotePackage",
                "chainId": "sui",
                "networkId": "mainnet",
                "packageRef": "0x1",
                "sourceUri": "https://example.invalid/graphql",
                "stateRef": "123",
            })
        );
    }

    #[test]
    fn manifest_carries_schema_and_router_hints() {
        let manifest = ToolManifest {
            id: "rust.static.scan_package".to_string(),
            version: "1".to_string(),
            chain: Some("sui".to_string()),
            category: "staticAnalysis".to_string(),
            description: "Run static analysis.".to_string(),
            when_to_use: vec!["Need deterministic source findings.".to_string()],
            when_not_to_use: vec!["Only explaining existing evidence.".to_string()],
            prerequisites: Vec::new(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            cost: ToolCost {
                risk: ToolRiskLevel::Low,
                expected_latency_ms: Some(1_000),
                token_budget_hint: Some(500),
                output_budget_tokens: Some(800),
            },
            action_class: ToolActionClass::ToolExecution,
            side_effects: Vec::new(),
            timeout_ms: Some(30_000),
            reducer_id: Some("staticAnalysis".to_string()),
            metadata: Metadata::new(),
        };

        let encoded = serde_json::to_value(manifest).unwrap();
        assert_eq!(encoded["reducerId"], "staticAnalysis");
        assert_eq!(encoded["inputSchema"]["type"], "object");
    }

    #[test]
    fn serializes_audit_session_packet() {
        let packet = AuditSessionPacket {
            schema_version: 1,
            id: "audit_1".to_string(),
            project: "demo".to_string(),
            repo_root: "/repo".to_string(),
            commit: "abc".to_string(),
            package_manifest: "Move.toml".to_string(),
            target_modules: vec!["vault".to_string()],
            compiler_version: Some("sui 1".to_string()),
            dependency_graph: Some(json!({"nodes": []})),
            selected_chain_adapter: "sui/move".to_string(),
            enabled_tools: vec!["rust.audit.run_full".to_string()],
            audit_profile: "full".to_string(),
            threat_model_profile: "default".to_string(),
            timestamp: "2026-05-24T00:00:00Z".to_string(),
            tool_versions: BTreeMap::new(),
            policy_profile: None,
            metadata: Metadata::new(),
        };

        let encoded = serde_json::to_value(packet).unwrap();
        assert_eq!(encoded["schemaVersion"], 1);
        assert_eq!(encoded["selectedChainAdapter"], "sui/move");
        assert_eq!(encoded["targetModules"][0], "vault");
    }
}
