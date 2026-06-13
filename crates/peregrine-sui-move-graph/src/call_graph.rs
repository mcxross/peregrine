use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSourceSpan {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCallGraph {
    pub nodes: Vec<MoveCallGraphNode>,
    pub edges: Vec<MoveCallGraphEdge>,
    pub unresolved_calls: Vec<MoveUnresolvedCall>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUnresolvedCall {
    pub source: String,
    pub raw_target: String,
    pub call_kind: String,
    pub file_path: String,
    pub spans: Vec<MoveSourceSpan>,
    pub reason: String,
}

pub(crate) fn finish_call_graph(
    mut nodes: Vec<MoveCallGraphNode>,
    mut edges: Vec<MoveCallGraphEdge>,
    mut unresolved_calls: Vec<MoveUnresolvedCall>,
) -> MoveCallGraph {
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    for edge in &mut edges {
        edge.source_spans.sort_by(compare_spans);
    }
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.raw_target.cmp(&right.raw_target))
    });

    unresolved_calls.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.raw_target.cmp(&right.raw_target))
    });

    MoveCallGraph {
        nodes,
        edges,
        unresolved_calls,
    }
}

pub(crate) fn function_id(
    package_path: Option<&str>,
    address: Option<&str>,
    module: &str,
    function: &str,
) -> String {
    format!(
        "function:{}:{}::{}::{}",
        package_path.unwrap_or("<external>"),
        address.unwrap_or("_"),
        module,
        function
    )
}

pub(crate) fn external_function_id(address: Option<&str>, module: &str, function: &str) -> String {
    format!(
        "external:function:{}::{}::{}",
        address.unwrap_or("_"),
        module,
        function
    )
}

pub(crate) fn unresolved_call_id(raw_target: &str) -> String {
    format!("unresolved:call:{raw_target}")
}

fn compare_spans(left: &MoveSourceSpan, right: &MoveSourceSpan) -> std::cmp::Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.start_byte.cmp(&right.start_byte))
        .then_with(|| left.end_byte.cmp(&right.end_byte))
}
