use peregrine_analysis::{AnalysisReport, GraphKind};
use peregrine_sui_mcp_protocol::{MoveProjectGraphs, MoveStateAccessGraph};
use rmcp::ErrorData;
use serde::de::DeserializeOwned;
use std::collections::HashSet;

pub(crate) fn legacy_project_graphs(
    report: &AnalysisReport,
) -> Result<MoveProjectGraphs, ErrorData> {
    Ok(MoveProjectGraphs {
        call_graph: legacy_graph(report, GraphKind::CALL).unwrap_or_default(),
        type_graph: legacy_graph(report, GraphKind::TYPE).unwrap_or_default(),
        state_access_graph: legacy_graph(report, GraphKind::STATE_ACCESS).unwrap_or_default(),
    })
}

pub(crate) fn legacy_state_graph(
    report: &AnalysisReport,
) -> Result<MoveStateAccessGraph, ErrorData> {
    legacy_graph(report, GraphKind::STATE_ACCESS)
}

fn legacy_graph<T>(report: &AnalysisReport, kind: &str) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    report
        .graphs
        .iter()
        .find(|graph| graph.kind.0 == kind)
        .and_then(|graph| graph.metadata.get("legacyGraph"))
        .cloned()
        .ok_or_else(|| {
            ErrorData::invalid_params(
                format!("analysis engine did not produce `{kind}` graph"),
                None,
            )
        })
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))
        })
}

pub(crate) fn filter_project_graphs(
    mut graphs: MoveProjectGraphs,
    modules: &[String],
    include_external: bool,
    _depth: Option<usize>,
) -> MoveProjectGraphs {
    if modules.is_empty() && include_external {
        return graphs;
    }

    let matches_modules = |module_name: Option<&str>, is_external: bool| -> bool {
        if !include_external && is_external {
            return false;
        }
        if modules.is_empty() {
            return true;
        }
        let Some(name) = module_name else {
            return false;
        };
        modules.iter().any(|m| {
            if m.contains("::") {
                name == m || name.contains(m)
            } else {
                name == m
            }
        })
    };

    let mut call_node_ids = HashSet::new();
    for node in &graphs.call_graph.nodes {
        if matches_modules(Some(&node.module_name), node.is_external) {
            call_node_ids.insert(node.id.clone());
        }
    }
    graphs
        .call_graph
        .edges
        .retain(|e| call_node_ids.contains(&e.source) && call_node_ids.contains(&e.target));
    for edge in &graphs.call_graph.edges {
        call_node_ids.insert(edge.source.clone());
        call_node_ids.insert(edge.target.clone());
    }
    graphs
        .call_graph
        .nodes
        .retain(|n| call_node_ids.contains(&n.id));
    graphs
        .call_graph
        .unresolved_calls
        .retain(|c| call_node_ids.contains(&c.source));

    let mut type_node_ids = HashSet::new();
    for node in &graphs.type_graph.nodes {
        if matches_modules(node.module_name.as_deref(), node.is_external) {
            type_node_ids.insert(node.id.clone());
        }
    }
    graphs
        .type_graph
        .edges
        .retain(|e| type_node_ids.contains(&e.source) && type_node_ids.contains(&e.target));
    for edge in &graphs.type_graph.edges {
        type_node_ids.insert(edge.source.clone());
        type_node_ids.insert(edge.target.clone());
    }
    graphs
        .type_graph
        .nodes
        .retain(|n| type_node_ids.contains(&n.id));
    graphs
        .type_graph
        .unresolved_types
        .retain(|c| type_node_ids.contains(&c.source));

    let mut state_node_ids = HashSet::new();
    for node in &graphs.state_access_graph.nodes {
        if matches_modules(node.module_name.as_deref(), node.is_external) {
            state_node_ids.insert(node.id.clone());
        }
    }
    graphs
        .state_access_graph
        .edges
        .retain(|e| state_node_ids.contains(&e.source) && state_node_ids.contains(&e.target));
    for edge in &graphs.state_access_graph.edges {
        state_node_ids.insert(edge.source.clone());
        state_node_ids.insert(edge.target.clone());
    }
    graphs
        .state_access_graph
        .nodes
        .retain(|n| state_node_ids.contains(&n.id));
    graphs
        .state_access_graph
        .unresolved_accesses
        .retain(|c| state_node_ids.contains(&c.source));

    graphs
}

pub(crate) fn render_project_graphs(graphs: &MoveProjectGraphs, format: &str) -> String {
    match format {
        "dot" => {
            let mut out = String::new();
            out.push_str("digraph peregrine_project_graphs {\n");
            for node in &graphs.call_graph.nodes {
                out.push_str(&format!(
                    "  \"{}\" [label=\"{}\"];\n",
                    node.id, node.qualified_name
                ));
            }
            for edge in &graphs.call_graph.edges {
                out.push_str(&format!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                    edge.source, edge.target, edge.call_kind
                ));
            }
            out.push_str("}\n");
            out
        }
        "mermaid" => {
            let mut out = String::new();
            out.push_str("graph TD;\n");
            for node in &graphs.call_graph.nodes {
                out.push_str(&format!(
                    "  {}[\"{}\"];\n",
                    node.id.replace("::", "_"),
                    node.qualified_name
                ));
            }
            for edge in &graphs.call_graph.edges {
                out.push_str(&format!(
                    "  {} -->|{}| {};\n",
                    edge.source.replace("::", "_"),
                    edge.call_kind,
                    edge.target.replace("::", "_")
                ));
            }
            out
        }
        "summary" => {
            format!(
                "Call Graph: {} nodes, {} edges\nType Graph: {} nodes, {} edges\nState Access Graph: {} nodes, {} edges",
                graphs.call_graph.nodes.len(),
                graphs.call_graph.edges.len(),
                graphs.type_graph.nodes.len(),
                graphs.type_graph.edges.len(),
                graphs.state_access_graph.nodes.len(),
                graphs.state_access_graph.edges.len()
            )
        }
        _ => "Unsupported format. Use 'json', 'dot', 'mermaid', or 'summary'.".to_string(),
    }
}
