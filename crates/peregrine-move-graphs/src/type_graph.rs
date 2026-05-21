use serde::Serialize;

use super::call_graph::MoveSourceSpan;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTypeGraph {
    pub nodes: Vec<MoveTypeGraphNode>,
    pub edges: Vec<MoveTypeGraphEdge>,
    pub unresolved_types: Vec<MoveUnresolvedType>,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTypeParameter {
    pub name: String,
    pub abilities: Vec<String>,
    pub is_phantom: bool,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUnresolvedType {
    pub source: String,
    pub raw_type: String,
    pub context: String,
    pub file_path: String,
    pub spans: Vec<MoveSourceSpan>,
    pub reason: String,
}

pub(crate) fn finish_type_graph(
    mut nodes: Vec<MoveTypeGraphNode>,
    mut edges: Vec<MoveTypeGraphEdge>,
    mut unresolved_types: Vec<MoveUnresolvedType>,
) -> MoveTypeGraph {
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    for edge in &mut edges {
        edge.source_spans.sort_by(compare_spans);
    }
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.relationship.cmp(&right.relationship))
    });

    unresolved_types.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.raw_type.cmp(&right.raw_type))
    });

    MoveTypeGraph {
        nodes,
        edges,
        unresolved_types,
    }
}

pub(crate) fn type_id(kind: &str, address: Option<&str>, module: &str, name: &str) -> String {
    format!(
        "type:{kind}:{}::{}::{}",
        address.unwrap_or("_"),
        module,
        name
    )
}

pub(crate) fn external_type_id(
    kind: &str,
    address: Option<&str>,
    module: &str,
    name: &str,
) -> String {
    format!(
        "external:type:{kind}:{}::{}::{}",
        address.unwrap_or("_"),
        module,
        name
    )
}

pub(crate) fn builtin_type_id(name: &str) -> String {
    format!("builtin:type:{name}")
}

pub(crate) fn type_parameter_id(owner_id: &str, name: &str) -> String {
    format!("typeParameter:{owner_id}::{name}")
}

fn compare_spans(left: &MoveSourceSpan, right: &MoveSourceSpan) -> std::cmp::Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.start_byte.cmp(&right.start_byte))
        .then_with(|| left.end_byte.cmp(&right.end_byte))
}
