use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

pub type AnalysisOptions = BTreeMap<String, Value>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChainId(pub String);

impl ChainId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginStage {
    Adapter,
    Scanner,
    GraphBuilder,
    StaticAnalyzer,
    DynamicAnalyzer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginOrigin {
    BuiltIn,
    Installed,
    Native,
    Wasm,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub id: String,
    pub version: String,
    pub chain: ChainId,
    pub stage: PluginStage,
    pub capabilities: Vec<String>,
    pub origin: PluginOrigin,
    pub priority: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AnalysisTarget {
    LocalPackage {
        path: PathBuf,
    },
    OnChainPackage {
        network: String,
        package_id: String,
        endpoint: Option<String>,
    },
    Transaction {
        network: String,
        digest: String,
        endpoint: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisLimits {
    pub max_artifacts: usize,
    pub max_graph_nodes: usize,
    pub max_graph_edges: usize,
    pub max_evidence_items: usize,
    pub max_output_bytes: usize,
    pub timeout_ms: u64,
}

impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            max_artifacts: 10_000,
            max_graph_nodes: 50_000,
            max_graph_edges: 100_000,
            max_evidence_items: 10_000,
            max_output_bytes: 256 * 1024,
            timeout_ms: 600_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTarget {
    pub chain: ChainId,
    pub target_id: String,
    pub package_root: Option<PathBuf>,
    pub metadata: Value,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisDiagnostic {
    pub stage: AnalysisStage,
    pub plugin_id: Option<String>,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl AnalysisDiagnostic {
    pub fn error(
        stage: AnalysisStage,
        plugin_id: impl Into<Option<String>>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            plugin_id: plugin_id.into(),
            severity: DiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn unavailable(
        stage: AnalysisStage,
        plugin_id: impl Into<Option<String>>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            plugin_id: plugin_id.into(),
            severity: DiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub source: String,
    pub artifact_id: Option<String>,
    pub span: Option<SourceSpan>,
    pub message: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub id: String,
    pub kind: String,
    pub qualified_name: String,
    pub span: Option<SourceSpan>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactBundle {
    pub chain: ChainId,
    pub target_id: String,
    pub package_root: Option<PathBuf>,
    pub artifacts: Vec<Artifact>,
    pub symbols: Vec<Symbol>,
    pub evidence: Vec<Evidence>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphKind(pub String);

impl GraphKind {
    pub const DEPENDENCY: &'static str = "dependency";
    pub const CALL: &'static str = "call";
    pub const CONTROL_FLOW: &'static str = "controlFlow";
    pub const DATA_FLOW: &'static str = "dataFlow";
    pub const TYPE: &'static str = "type";
    pub const STATE_ACCESS: &'static str = "stateAccess";

    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn required() -> Vec<Self> {
        [
            Self::DEPENDENCY,
            Self::CALL,
            Self::CONTROL_FLOW,
            Self::DATA_FLOW,
        ]
        .into_iter()
        .map(Self::new)
        .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub span: Option<SourceSpan>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub spans: Vec<SourceSpan>,
    pub evidence: Vec<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyGraph {
    pub kind: GraphKind,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub metadata: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub analyzer_id: String,
    pub ruleset_id: Option<String>,
    pub rule_id: String,
    pub severity: FindingSeverity,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub evidence: Vec<Evidence>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisMetric {
    pub analyzer_id: String,
    pub name: String,
    pub value: Value,
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticAnalysisOutput {
    pub findings: Vec<Finding>,
    pub metrics: Vec<AnalysisMetric>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub metadata: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicResultStatus {
    Completed,
    Failed,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicAnalysisOutput {
    pub analyzer_id: String,
    pub capability: String,
    pub status: DynamicResultStatus,
    pub result: Value,
    pub evidence: Vec<Evidence>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisStage {
    Adapter,
    Scan,
    Graph,
    Static,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageStatus {
    Passed,
    Partial,
    Failed,
    Skipped,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageReport {
    pub stage: AnalysisStage,
    pub status: StageStatus,
    pub plugin_ids: Vec<String>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRequest {
    pub chain: ChainId,
    pub target: AnalysisTarget,
    pub stages: Vec<AnalysisStage>,
    pub graph_kinds: Vec<GraphKind>,
    pub plugin_ids: Vec<String>,
    pub dynamic_capabilities: Vec<String>,
    pub limits: AnalysisLimits,
    pub options: AnalysisOptions,
}

impl AnalysisRequest {
    pub fn safe(chain: ChainId, target: AnalysisTarget) -> Self {
        Self {
            chain,
            target,
            stages: vec![
                AnalysisStage::Scan,
                AnalysisStage::Graph,
                AnalysisStage::Static,
            ],
            graph_kinds: GraphKind::required(),
            plugin_ids: Vec::new(),
            dynamic_capabilities: Vec::new(),
            limits: AnalysisLimits::default(),
            options: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub chain: ChainId,
    pub target_id: Option<String>,
    pub artifacts: Option<ArtifactBundle>,
    pub graphs: Vec<PropertyGraph>,
    pub findings: Vec<Finding>,
    pub metrics: Vec<AnalysisMetric>,
    pub dynamic_results: Vec<DynamicAnalysisOutput>,
    pub stages: Vec<StageReport>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub selected_plugins: Vec<String>,
}

impl AnalysisReport {
    pub fn empty(chain: ChainId) -> Self {
        Self {
            chain,
            target_id: None,
            artifacts: None,
            graphs: Vec::new(),
            findings: Vec::new(),
            metrics: Vec::new(),
            dynamic_results: Vec::new(),
            stages: Vec::new(),
            diagnostics: Vec::new(),
            selected_plugins: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterPackage {
    pub id: String,
    pub root: Option<PathBuf>,
    pub bytes: Vec<u8>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterTransaction {
    pub digest: String,
    pub bytes: Vec<u8>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEnvironment {
    pub id: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainOperation {
    pub kind: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainOperationResult {
    pub status: String,
    pub output: Value,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisError {
    pub code: String,
    pub message: String,
}

impl AnalysisError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AnalysisError {}
