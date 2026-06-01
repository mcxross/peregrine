use super::{
    common::{DIM, EDGE, FUNCTION, HEADER, KIND, MODULE, RESET, graph_step, requested_modules},
    dot::{DotEdgeStyle, dot_edge_attrs, dot_id, dot_label},
    project::{module_matches, selected_source_package},
};
use crate::{
    output::{CliDiagnostic, CliStep},
    sui::{args::CallGraphArgs, project::CliContext},
};
use peregrine_move_graphs::{
    MoveCallGraph, MoveCallGraphEdge, MoveCallGraphNode, MoveUnresolvedCall,
    discover_move_project_graphs_for_package,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

pub fn run_call_graph(context: &CliContext, args: &CallGraphArgs) -> CliStep {
    let started_at = Instant::now();
    if let Err(error) = selected_source_package(context, "call-graph") {
        return CliStep::failed("call-graph", started_at, error);
    }

    let graph =
        discover_move_project_graphs_for_package(&context.project_root, &context.package_path)
            .call_graph;
    let graph = filter_call_graph(graph, args);

    if graph.nodes.is_empty() {
        return CliStep::failed(
            "call-graph",
            started_at,
            CliDiagnostic::error(
                "call-graph",
                "No call graph nodes matched the requested target.",
            ),
        );
    }

    let rendered = if args.output.dot {
        render_call_graph_dot(&graph)
    } else {
        render_call_graph_text(&graph)
    };

    graph_step(
        "call-graph",
        started_at,
        display_command(args),
        context,
        &args.output,
        rendered,
        BTreeMap::from([
            ("nodeCount".to_string(), json!(graph.nodes.len())),
            ("edgeCount".to_string(), json!(graph.edges.len())),
            (
                "unresolvedCallCount".to_string(),
                json!(graph.unresolved_calls.len()),
            ),
        ]),
        json!({ "graph": graph }),
    )
}

fn filter_call_graph(graph: MoveCallGraph, args: &CallGraphArgs) -> MoveCallGraph {
    let requested_modules = requested_modules(&args.modules);
    let mut node_ids = graph
        .nodes
        .iter()
        .filter(|node| args.include_external || !node.is_external)
        .filter(|node| {
            requested_modules.is_empty()
                || requested_modules.iter().any(|requested| {
                    module_matches(requested, node.address.as_deref(), &node.module_name)
                })
        })
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    let edges = graph
        .edges
        .into_iter()
        .filter(|edge| node_ids.contains(&edge.source) && node_ids.contains(&edge.target))
        .collect::<Vec<_>>();

    let unresolved_calls = graph
        .unresolved_calls
        .into_iter()
        .filter(|call| node_ids.contains(&call.source))
        .collect::<Vec<_>>();

    for edge in &edges {
        node_ids.insert(edge.source.clone());
        node_ids.insert(edge.target.clone());
    }

    let nodes = graph
        .nodes
        .into_iter()
        .filter(|node| node_ids.contains(&node.id))
        .collect::<Vec<_>>();

    MoveCallGraph {
        nodes,
        edges,
        unresolved_calls,
    }
}

fn render_call_graph_text(graph: &MoveCallGraph) -> String {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&MoveCallGraphEdge>>::new();
    let mut unresolved = BTreeMap::<&str, Vec<&MoveUnresolvedCall>>::new();

    for edge in &graph.edges {
        outgoing.entry(edge.source.as_str()).or_default().push(edge);
    }
    for call in &graph.unresolved_calls {
        unresolved
            .entry(call.source.as_str())
            .or_default()
            .push(call);
    }

    let mut modules = BTreeMap::<String, Vec<&MoveCallGraphNode>>::new();
    for node in &graph.nodes {
        modules.entry(module_label(node)).or_default().push(node);
    }

    let mut lines = vec![format!(
        "{HEADER}call graph{RESET} {DIM}nodes={} edges={} unresolved={}{RESET}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.unresolved_calls.len()
    )];

    for (module, mut module_nodes) in modules {
        module_nodes.sort_by(|left, right| left.function_name.cmp(&right.function_name));
        lines.push(format!("{DIM}|--{RESET} {MODULE}module{RESET} {module}"));

        for node in module_nodes {
            let entry = if node.is_entry { " entry" } else { "" };
            let external = if node.is_external { " external" } else { "" };
            lines.push(format!(
                "{DIM}|   |--{RESET} {FUNCTION}{}{RESET} {DIM}[{}{}{}]{RESET}",
                node.function_name, node.visibility, entry, external
            ));

            for edge in outgoing.get(node.id.as_str()).into_iter().flatten() {
                let target = nodes
                    .get(edge.target.as_str())
                    .map(|target| target.qualified_name.as_str())
                    .unwrap_or(edge.raw_target.as_str());
                lines.push(format!(
                    "{DIM}|   |   |--{RESET} {EDGE}calls{RESET} {target} {KIND}{} x{}{RESET}",
                    edge.call_kind, edge.call_count
                ));
            }

            for call in unresolved.get(node.id.as_str()).into_iter().flatten() {
                lines.push(format!(
                    "{DIM}|   |   |--{RESET} {EDGE}unresolved{RESET} {} {KIND}{}{RESET}",
                    call.raw_target, call.reason
                ));
            }
        }
    }

    lines.join("\n")
}

fn render_call_graph_dot(graph: &MoveCallGraph) -> String {
    let mut lines = vec![
        "digraph peregrine_call_graph {".to_string(),
        "  graph [rankdir=LR, bgcolor=\"transparent\"];".to_string(),
        "  node [shape=box, style=\"rounded,filled\", fontname=\"Menlo\", fontsize=10];"
            .to_string(),
        "  edge [fontname=\"Menlo\", fontsize=9];".to_string(),
    ];

    for node in &graph.nodes {
        let fill = if node.is_external {
            "#2d2d2d"
        } else if node.is_entry {
            "#064e3b"
        } else {
            "#1f2937"
        };
        let font = if node.is_external {
            "#d1d5db"
        } else {
            "#f9fafb"
        };
        lines.push(format!(
            "  {} [label={}, fillcolor=\"{}\", fontcolor=\"{}\"];",
            dot_id(&node.id),
            dot_label(&format!("{}\n{}", node.qualified_name, node.visibility)),
            fill,
            font
        ));
    }

    for edge in &graph.edges {
        let attrs = dot_edge_attrs(
            &format!("{} x{}", edge.call_kind, edge.call_count),
            call_edge_style(edge),
        );
        lines.push(format!(
            "  {} -> {} [{}];",
            dot_id(&edge.source),
            dot_id(&edge.target),
            attrs
        ));
    }

    for call in &graph.unresolved_calls {
        let unresolved_id = format!("unresolved:{}:{}", call.source, call.raw_target);
        lines.push(format!(
            "  {} [label={}, shape=note, fillcolor=\"#451a03\", fontcolor=\"#fed7aa\"];",
            dot_id(&unresolved_id),
            dot_label(&format!("unresolved\n{}", call.raw_target))
        ));
        lines.push(format!(
            "  {} -> {} [{}];",
            dot_id(&call.source),
            dot_id(&unresolved_id),
            dot_edge_attrs(&call.call_kind, UNRESOLVED_EDGE)
        ));
    }

    lines.push("}".to_string());
    lines.join("\n")
}

const DIRECT_CALL_EDGE: DotEdgeStyle = DotEdgeStyle::new("#22c55e", "#bbf7d0", "solid", "1.8");
const METHOD_CALL_EDGE: DotEdgeStyle = DotEdgeStyle::new("#38bdf8", "#bae6fd", "solid", "1.8");
const MACRO_CALL_EDGE: DotEdgeStyle = DotEdgeStyle::new("#c084fc", "#e9d5ff", "bold", "2.2");
const EXTERNAL_CALL_EDGE: DotEdgeStyle = DotEdgeStyle::new("#94a3b8", "#cbd5e1", "dashed", "1.4");
const LOW_CONFIDENCE_CALL_EDGE: DotEdgeStyle =
    DotEdgeStyle::new("#f59e0b", "#fde68a", "dotted", "1.4");
const UNRESOLVED_EDGE: DotEdgeStyle = DotEdgeStyle::new("#fb923c", "#fed7aa", "dashed", "1.6");

fn call_edge_style(edge: &MoveCallGraphEdge) -> DotEdgeStyle {
    if !edge.is_resolved || edge.confidence == "low" {
        return LOW_CONFIDENCE_CALL_EDGE;
    }
    if edge.is_external {
        return EXTERNAL_CALL_EDGE;
    }

    match edge.call_kind.as_str() {
        "direct" => DIRECT_CALL_EDGE,
        "method" => METHOD_CALL_EDGE,
        "macro" | "methodMacro" => MACRO_CALL_EDGE,
        _ => DIRECT_CALL_EDGE,
    }
}

fn module_label(node: &MoveCallGraphNode) -> String {
    node.address
        .as_deref()
        .map(|address| format!("{address}::{}", node.module_name))
        .unwrap_or_else(|| node.module_name.clone())
}

fn display_command(args: &CallGraphArgs) -> String {
    let mut command = "peregrine call-graph".to_string();

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

    #[test]
    fn call_graph_dot_contains_edges() {
        let graph = MoveCallGraph {
            nodes: vec![node("a", "m", "a"), node("b", "m", "b")],
            edges: vec![MoveCallGraphEdge {
                source: "a".to_string(),
                target: "b".to_string(),
                call_kind: "direct".to_string(),
                confidence: "high".to_string(),
                call_count: 2,
                raw_target: "b".to_string(),
                type_arguments: Vec::new(),
                source_spans: Vec::new(),
                is_external: false,
                is_resolved: true,
            }],
            unresolved_calls: Vec::new(),
        };

        let rendered = render_call_graph_dot(&graph);

        assert!(rendered.contains("digraph peregrine_call_graph"));
        assert!(rendered.contains("\"a\" -> \"b\""));
        assert!(rendered.contains("direct x2"));
        assert!(rendered.contains("color=\"#22c55e\""));
    }

    fn node(id: &str, module: &str, function: &str) -> MoveCallGraphNode {
        MoveCallGraphNode {
            id: id.to_string(),
            package_name: Some("pkg".to_string()),
            package_path: Some(".".to_string()),
            address: Some("pkg".to_string()),
            module_name: module.to_string(),
            function_name: function.to_string(),
            qualified_name: format!("pkg::{module}::{function}"),
            file_path: None,
            visibility: "public".to_string(),
            is_entry: false,
            is_transaction_callable: true,
            attributes: Vec::new(),
            signature: None,
            span: None,
            is_external: false,
            source: "source".to_string(),
        }
    }
}
