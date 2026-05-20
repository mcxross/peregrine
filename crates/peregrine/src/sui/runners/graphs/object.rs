use super::{
    common::{graph_step, requested_modules, DIM, EDGE, FUNCTION, HEADER, KIND, MODULE, RESET},
    dot::{dot_edge_attrs, dot_id, dot_label, DotEdgeStyle},
    project::module_matches,
};
use crate::{
    output::{CliDiagnostic, CliStep},
    sui::{args::ObjectGraphArgs, project::CliContext},
};
use peregrine_static_analysis::{
    discover_project_graphs_for_package, MoveStateAccessGraph, MoveStateAccessGraphEdge,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

pub fn run_object_graph(context: &CliContext, args: &ObjectGraphArgs) -> CliStep {
    let started_at = Instant::now();
    let graph = discover_project_graphs_for_package(&context.project_root, &context.package_path)
        .state_access_graph;
    let graph = filter_object_graph(graph, args);

    if graph.nodes.is_empty() {
        return CliStep::failed(
            "object-graph",
            started_at,
            CliDiagnostic::error(
                "object-graph",
                "No object graph nodes matched the requested target.",
            ),
        );
    }

    let rendered = if args.output.dot {
        render_object_graph_dot(&graph)
    } else {
        render_object_graph_text(&graph)
    };

    graph_step(
        "object-graph",
        started_at,
        display_command(args),
        context,
        &args.output,
        rendered,
        BTreeMap::from([
            ("nodeCount".to_string(), json!(graph.nodes.len())),
            ("edgeCount".to_string(), json!(graph.edges.len())),
            (
                "unresolvedAccessCount".to_string(),
                json!(graph.unresolved_accesses.len()),
            ),
        ]),
        json!({ "graph": graph }),
    )
}

fn filter_object_graph(
    graph: MoveStateAccessGraph,
    args: &ObjectGraphArgs,
) -> MoveStateAccessGraph {
    let requested_modules = requested_modules(&args.modules);
    let mut seed_ids = graph
        .nodes
        .iter()
        .filter(|node| args.include_external || !node.is_external)
        .filter(|node| {
            requested_modules.is_empty()
                || requested_modules.iter().any(|requested| {
                    module_matches(
                        requested,
                        node.address.as_deref(),
                        node.module_name.as_deref().unwrap_or_default(),
                    )
                })
        })
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    let edges = graph
        .edges
        .into_iter()
        .filter(|edge| seed_ids.contains(&edge.source) || seed_ids.contains(&edge.target))
        .collect::<Vec<_>>();

    for edge in &edges {
        seed_ids.insert(edge.source.clone());
        seed_ids.insert(edge.target.clone());
    }

    let nodes = graph
        .nodes
        .into_iter()
        .filter(|node| seed_ids.contains(&node.id))
        .filter(|node| args.include_external || !node.is_external)
        .collect::<Vec<_>>();
    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let edges = edges
        .into_iter()
        .filter(|edge| node_ids.contains(&edge.source) && node_ids.contains(&edge.target))
        .collect::<Vec<_>>();
    let unresolved_accesses = graph
        .unresolved_accesses
        .into_iter()
        .filter(|access| node_ids.contains(&access.source))
        .collect::<Vec<_>>();

    MoveStateAccessGraph {
        nodes,
        edges,
        unresolved_accesses,
    }
}

fn render_object_graph_text(graph: &MoveStateAccessGraph) -> String {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut object_nodes = graph
        .nodes
        .iter()
        .filter(|node| node.kind != "function")
        .collect::<Vec<_>>();
    let mut function_edges = BTreeMap::<&str, Vec<&MoveStateAccessGraphEdge>>::new();

    object_nodes.sort_by(|left, right| left.qualified_name.cmp(&right.qualified_name));
    for edge in &graph.edges {
        function_edges
            .entry(edge.source.as_str())
            .or_default()
            .push(edge);
    }

    let mut lines = vec![format!(
        "{HEADER}object graph{RESET} {DIM}nodes={} edges={} unresolved={}{RESET}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.unresolved_accesses.len()
    )];

    lines.push(format!("{DIM}|--{RESET} {MODULE}objects{RESET}"));
    for node in object_nodes {
        lines.push(format!(
            "{DIM}|   |--{RESET} {KIND}{}{RESET} {}",
            node.kind, node.qualified_name
        ));
    }

    lines.push(format!("{DIM}|--{RESET} {MODULE}accesses{RESET}"));
    for node in graph.nodes.iter().filter(|node| node.kind == "function") {
        let Some(edges) = function_edges.get(node.id.as_str()) else {
            continue;
        };
        lines.push(format!(
            "{DIM}|   |--{RESET} {FUNCTION}{}{RESET}",
            node.qualified_name
        ));

        for edge in edges {
            let target_node = nodes.get(edge.target.as_str()).copied();
            let target = target_node
                .map(|target| target.qualified_name.as_str())
                .unwrap_or(edge.target.as_str());
            let field = match (target_node, edge.field_name.as_deref()) {
                (Some(target), Some(_)) if target.kind == "field" => String::new(),
                (_, Some(field)) => format!(" .{field}"),
                _ => String::new(),
            };
            lines.push(format!(
                "{DIM}|   |   |--{RESET} {EDGE}{}{RESET} {target}{field} {KIND}{}{RESET}",
                edge.access_kind, edge.confidence
            ));
        }
    }

    if !graph.unresolved_accesses.is_empty() {
        lines.push(format!("{DIM}|--{RESET} {MODULE}unresolved{RESET}"));
        for access in &graph.unresolved_accesses {
            lines.push(format!(
                "{DIM}|   |--{RESET} {EDGE}{}{RESET} {} {KIND}{}{RESET}",
                access.access_kind, access.raw_target, access.reason
            ));
        }
    }

    lines.join("\n")
}

fn render_object_graph_dot(graph: &MoveStateAccessGraph) -> String {
    let mut lines = vec![
        "digraph peregrine_object_graph {".to_string(),
        "  graph [rankdir=LR, bgcolor=\"transparent\"];".to_string(),
        "  node [style=\"rounded,filled\", fontname=\"Menlo\", fontsize=10];".to_string(),
        "  edge [fontname=\"Menlo\", fontsize=9];".to_string(),
    ];

    for node in &graph.nodes {
        let (shape, fill) = match node.kind.as_str() {
            "function" => ("box", "#1e293b"),
            "field" => ("ellipse", "#3b0764"),
            "stateType" => ("component", "#064e3b"),
            _ => ("box", "#27272a"),
        };
        lines.push(format!(
            "  {} [label={}, shape={}, fillcolor=\"{}\", fontcolor=\"#f8fafc\"];",
            dot_id(&node.id),
            dot_label(&format!("{}\n{}", node.qualified_name, node.kind)),
            shape,
            fill
        ));
    }

    for edge in &graph.edges {
        let label = edge
            .field_name
            .as_deref()
            .map(|field| format!("{} .{field}", edge.access_kind))
            .unwrap_or_else(|| edge.access_kind.clone());
        lines.push(format!(
            "  {} -> {} [{}];",
            dot_id(&edge.source),
            dot_id(&edge.target),
            dot_edge_attrs(&label, object_edge_style(edge.access_kind.as_str()))
        ));
    }

    for access in &graph.unresolved_accesses {
        let unresolved_id = format!("unresolved:{}:{}", access.source, access.raw_target);
        lines.push(format!(
            "  {} [label={}, shape=note, fillcolor=\"#451a03\", fontcolor=\"#fed7aa\"];",
            dot_id(&unresolved_id),
            dot_label(&format!("unresolved\n{}", access.raw_target))
        ));
        lines.push(format!(
            "  {} -> {} [{}];",
            dot_id(&access.source),
            dot_id(&unresolved_id),
            dot_edge_attrs(&access.access_kind, UNRESOLVED_EDGE)
        ));
    }

    lines.push("}".to_string());
    lines.join("\n")
}

const READ_EDGE: DotEdgeStyle = DotEdgeStyle::new("#38bdf8", "#bae6fd", "solid", "1.7");
const WRITE_EDGE: DotEdgeStyle = DotEdgeStyle::new("#f97316", "#fed7aa", "bold", "2.2");
const BORROW_IMM_EDGE: DotEdgeStyle = DotEdgeStyle::new("#2dd4bf", "#ccfbf1", "solid", "1.7");
const BORROW_MUT_EDGE: DotEdgeStyle = DotEdgeStyle::new("#ec4899", "#fbcfe8", "bold", "2.2");
const MOVE_EDGE: DotEdgeStyle = DotEdgeStyle::new("#eab308", "#fef08a", "solid", "1.7");
const CALL_EDGE: DotEdgeStyle = DotEdgeStyle::new("#a3e635", "#d9f99d", "dashed", "1.5");
const UNRESOLVED_EDGE: DotEdgeStyle = DotEdgeStyle::new("#fb923c", "#fed7aa", "dashed", "1.6");
const OTHER_ACCESS_EDGE: DotEdgeStyle = DotEdgeStyle::new("#94a3b8", "#cbd5e1", "solid", "1.4");

fn object_edge_style(access_kind: &str) -> DotEdgeStyle {
    match access_kind {
        "read" => READ_EDGE,
        "write" => WRITE_EDGE,
        "borrowImm" => BORROW_IMM_EDGE,
        "borrowMut" => BORROW_MUT_EDGE,
        "move" => MOVE_EDGE,
        "call" => CALL_EDGE,
        _ => OTHER_ACCESS_EDGE,
    }
}

fn display_command(args: &ObjectGraphArgs) -> String {
    let mut command = "peregrine object-graph".to_string();

    for module in &args.modules {
        command.push_str(&format!(" --module {module}"));
    }
    if args.include_external {
        command.push_str(" --include-external");
    }
    if args.output.dot {
        command.push_str(" --dot");
    }
    if let Some(output) = &args.output.output {
        command.push_str(&format!(" --output {}", output.display()));
    }

    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_static_analysis::MoveStateAccessGraphNode;

    #[test]
    fn object_graph_text_lists_accesses() {
        let graph = MoveStateAccessGraph {
            nodes: vec![
                node("f", "function", "pkg::m::deposit"),
                node("o", "stateType", "pkg::m::Vault"),
            ],
            edges: vec![MoveStateAccessGraphEdge {
                source: "f".to_string(),
                target: "o".to_string(),
                access_kind: "borrowMut".to_string(),
                field_name: None,
                via_function: None,
                source_spans: Vec::new(),
                confidence: "high".to_string(),
                evidence: Vec::new(),
            }],
            unresolved_accesses: Vec::new(),
        };

        let rendered = render_object_graph_text(&graph);

        assert!(rendered.contains("object graph"));
        assert!(rendered.contains("pkg::m::deposit"));
        assert!(rendered.contains("borrowMut"));
    }

    #[test]
    fn object_graph_dot_colors_edges_by_access_kind() {
        let graph = MoveStateAccessGraph {
            nodes: vec![
                node("f", "function", "pkg::m::deposit"),
                node("o", "stateType", "pkg::m::Vault"),
            ],
            edges: vec![MoveStateAccessGraphEdge {
                source: "f".to_string(),
                target: "o".to_string(),
                access_kind: "borrowMut".to_string(),
                field_name: None,
                via_function: None,
                source_spans: Vec::new(),
                confidence: "high".to_string(),
                evidence: Vec::new(),
            }],
            unresolved_accesses: Vec::new(),
        };

        let rendered = render_object_graph_dot(&graph);

        assert!(rendered.contains("borrowMut"));
        assert!(rendered.contains("color=\"#ec4899\""));
    }

    fn node(id: &str, kind: &str, qualified_name: &str) -> MoveStateAccessGraphNode {
        MoveStateAccessGraphNode {
            id: id.to_string(),
            kind: kind.to_string(),
            package_name: Some("pkg".to_string()),
            package_path: Some(".".to_string()),
            address: Some("pkg".to_string()),
            module_name: Some("m".to_string()),
            name: qualified_name.rsplit("::").next().unwrap().to_string(),
            qualified_name: qualified_name.to_string(),
            file_path: None,
            abilities: Vec::new(),
            span: None,
            is_external: false,
            source: "source".to_string(),
        }
    }
}
