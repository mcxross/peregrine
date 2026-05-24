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
    FunctionRiskMap,
    Invariants,
    StaticAnalysis,
    GraphAnalysis,
    BytecodeReview,
    AttackHypotheses,
    TargetedTests,
    DynamicAnalysis,
    InvariantStress,
    ExploitConfirmation,
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
        assert_eq!(json!(AuditFixState::PartiallyFixed), json!("partiallyFixed"));
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

        let encoded = serde_json::to_value(manifest).expect("manifest json");
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

        let encoded = serde_json::to_value(packet).expect("audit session json");
        assert_eq!(encoded["schemaVersion"], 1);
        assert_eq!(encoded["selectedChainAdapter"], "sui/move");
        assert_eq!(encoded["targetModules"][0], "vault");
    }
}
