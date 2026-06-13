use crate::PackageSummary;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSourceSpan {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProjectGraphs {
    pub call_graph: MoveCallGraph,
    pub type_graph: MoveTypeGraph,
    pub state_access_graph: MoveStateAccessGraph,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCallGraph {
    pub nodes: Vec<MoveCallGraphNode>,
    pub edges: Vec<MoveCallGraphEdge>,
    pub unresolved_calls: Vec<MoveUnresolvedCall>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCallGraphNode {
    pub id: String,
    pub package_name: Option<String>,
    pub package_path: Option<String>,
    pub address: Option<String>,
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub file_path: Option<String>,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub attributes: Vec<String>,
    pub signature: Option<String>,
    pub span: Option<MoveSourceSpan>,
    pub is_external: bool,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCallGraphEdge {
    pub source: String,
    pub target: String,
    pub call_kind: String,
    pub confidence: String,
    pub call_count: usize,
    pub raw_target: String,
    pub type_arguments: Vec<String>,
    pub source_spans: Vec<MoveSourceSpan>,
    pub is_external: bool,
    pub is_resolved: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUnresolvedCall {
    pub source: String,
    pub raw_target: String,
    pub call_kind: String,
    pub file_path: String,
    pub spans: Vec<MoveSourceSpan>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTypeGraph {
    pub nodes: Vec<MoveTypeGraphNode>,
    pub edges: Vec<MoveTypeGraphEdge>,
    pub unresolved_types: Vec<MoveUnresolvedType>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTypeGraphNode {
    pub id: String,
    pub kind: String,
    pub package_name: Option<String>,
    pub package_path: Option<String>,
    pub address: Option<String>,
    pub canonical_address: Option<String>,
    pub module_name: Option<String>,
    pub name: String,
    pub qualified_name: String,
    pub file_path: Option<String>,
    pub abilities: Vec<String>,
    pub type_parameters: Vec<MoveTypeParameter>,
    pub attributes: Vec<String>,
    pub span: Option<MoveSourceSpan>,
    pub source: String,
    pub is_external: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTypeParameter {
    pub name: String,
    pub abilities: Vec<String>,
    pub is_phantom: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTypeGraphEdge {
    pub source: String,
    pub target: String,
    pub relationship: String,
    pub field_name: Option<String>,
    pub variant_name: Option<String>,
    pub function_name: Option<String>,
    pub parameter_name: Option<String>,
    pub type_argument_index: Option<usize>,
    pub is_mutable: bool,
    pub is_reference: bool,
    pub type_expression: Option<String>,
    pub declaring_type_id: Option<String>,
    pub declaring_field_name: Option<String>,
    pub type_argument_name: Option<String>,
    pub source_spans: Vec<MoveSourceSpan>,
    pub confidence: String,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUnresolvedType {
    pub source: String,
    pub raw_type: String,
    pub context: String,
    pub file_path: String,
    pub spans: Vec<MoveSourceSpan>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStateAccessGraph {
    pub nodes: Vec<MoveStateAccessGraphNode>,
    pub edges: Vec<MoveStateAccessGraphEdge>,
    pub unresolved_accesses: Vec<MoveUnresolvedStateAccess>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStateAccessGraphNode {
    pub id: String,
    pub kind: String,
    pub package_name: Option<String>,
    pub package_path: Option<String>,
    pub address: Option<String>,
    pub module_name: Option<String>,
    pub name: String,
    pub qualified_name: String,
    pub file_path: Option<String>,
    pub abilities: Vec<String>,
    pub span: Option<MoveSourceSpan>,
    pub is_external: bool,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStateAccessGraphEdge {
    pub source: String,
    pub target: String,
    pub access_kind: String,
    pub field_name: Option<String>,
    pub via_function: Option<String>,
    pub source_spans: Vec<MoveSourceSpan>,
    pub confidence: String,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUnresolvedStateAccess {
    pub source: String,
    pub raw_target: String,
    pub access_kind: String,
    pub file_path: String,
    pub spans: Vec<MoveSourceSpan>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphsResponse {
    pub status: String,
    pub package: PackageSummary,
    pub graphs: MoveProjectGraphs,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionStateGraphResponse {
    pub status: String,
    pub package: PackageSummary,
    pub graph: MoveStateAccessGraph,
}
