use serde::{Deserialize, Serialize};

use super::call_graph::MoveSourceSpan;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStateAccessGraph {
    pub nodes: Vec<MoveStateAccessGraphNode>,
    pub edges: Vec<MoveStateAccessGraphEdge>,
    pub unresolved_accesses: Vec<MoveUnresolvedStateAccess>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUnresolvedStateAccess {
    pub source: String,
    pub raw_target: String,
    pub access_kind: String,
    pub file_path: String,
    pub spans: Vec<MoveSourceSpan>,
    pub reason: String,
}

pub(crate) fn finish_state_access_graph(
    mut nodes: Vec<MoveStateAccessGraphNode>,
    mut edges: Vec<MoveStateAccessGraphEdge>,
    mut unresolved_accesses: Vec<MoveUnresolvedStateAccess>,
) -> MoveStateAccessGraph {
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    for edge in &mut edges {
        edge.source_spans.sort_by(compare_spans);
        edge.evidence.sort();
        edge.evidence.dedup();
    }
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.access_kind.cmp(&right.access_kind))
            .then_with(|| left.field_name.cmp(&right.field_name))
    });

    unresolved_accesses.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.raw_target.cmp(&right.raw_target))
            .then_with(|| left.access_kind.cmp(&right.access_kind))
    });

    MoveStateAccessGraph {
        nodes,
        edges,
        unresolved_accesses,
    }
}

pub(crate) fn state_field_id(owner_type_id: &str, field_name: &str) -> String {
    format!("stateField:{owner_type_id}::{field_name}")
}

fn compare_spans(left: &MoveSourceSpan, right: &MoveSourceSpan) -> std::cmp::Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.start_byte.cmp(&right.start_byte))
        .then_with(|| left.end_byte.cmp(&right.end_byte))
}
