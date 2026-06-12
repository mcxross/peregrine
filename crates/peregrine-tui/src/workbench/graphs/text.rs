use super::pane::GraphDocument;
use crate::output::{CliStatus, CliStep};
use crate::sui::args::GraphOutputArgs;
use crate::workbench::WorkbenchTab;
use peregrine_mcp_protocol::{
    MoveTypeGraph, MoveTypeGraphEdge, MoveTypeGraphNode, MoveUnresolvedType,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn text_graph_output_args() -> GraphOutputArgs {
    GraphOutputArgs {
        dot: false,
        output: None,
    }
}

pub(crate) fn graph_step_document(tab: WorkbenchTab, step: CliStep) -> Result<GraphDocument, String> {
    if step.status != CliStatus::Passed {
        return Err(render_graph_step_error(&step));
    }

    Ok(GraphDocument::new(
        tab.title(),
        strip_ansi_sequences(step.stdout.trim_end()),
    ))
}

fn render_graph_step_error(step: &CliStep) -> String {
    let mut lines = Vec::new();

    for diagnostic in &step.diagnostics {
        lines.push(format!("{}: {}", diagnostic.source, diagnostic.message));
    }

    if !step.stdout.trim().is_empty() {
        lines.push("stdout:".to_string());
        lines.extend(
            strip_ansi_sequences(step.stdout.trim_end())
                .lines()
                .map(|line| format!("  {line}")),
        );
    }

    if !step.stderr.trim().is_empty() {
        lines.push("stderr:".to_string());
        lines.extend(
            strip_ansi_sequences(step.stderr.trim_end())
                .lines()
                .map(|line| format!("  {line}")),
        );
    }

    if lines.is_empty() {
        lines.push(format!("{} failed.", step.name));
    }

    lines.join("\n")
}

fn strip_ansi_sequences(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            output.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for code in chars.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                let mut escaped = false;
                for code in chars.by_ref() {
                    if escaped && code == '\\' {
                        break;
                    }
                    escaped = code == '\x1b';
                    if code == '\x07' {
                        break;
                    }
                }
            }
            Some(_) | None => {}
        }
    }

    output
}

pub(crate) fn filter_type_graph(graph: MoveTypeGraph, module_filters: &[String]) -> MoveTypeGraph {
    let requested_modules = module_filters
        .iter()
        .map(|module| module.trim())
        .filter(|module| !module.is_empty())
        .collect::<Vec<_>>();
    let selected_ids = graph
        .nodes
        .iter()
        .filter(|node| !node.is_external)
        .filter(|node| {
            requested_modules.is_empty()
                || requested_modules
                    .iter()
                    .any(|requested| type_graph_node_matches(node, requested))
        })
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    if selected_ids.is_empty() {
        return MoveTypeGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_types: Vec::new(),
        };
    }

    let mut node_ids = selected_ids.clone();
    let mut edges = Vec::new();
    for edge in graph.edges {
        if selected_ids.contains(&edge.source) || selected_ids.contains(&edge.target) {
            node_ids.insert(edge.source.clone());
            node_ids.insert(edge.target.clone());
            edges.push(edge);
        }
    }

    let unresolved_types = graph
        .unresolved_types
        .into_iter()
        .filter(|unresolved| selected_ids.contains(&unresolved.source))
        .collect::<Vec<_>>();
    let nodes = graph
        .nodes
        .into_iter()
        .filter(|node| node_ids.contains(&node.id))
        .collect::<Vec<_>>();

    MoveTypeGraph {
        nodes,
        edges,
        unresolved_types,
    }
}

fn type_graph_node_matches(node: &MoveTypeGraphNode, requested: &str) -> bool {
    let Some(module_name) = node.module_name.as_deref() else {
        return false;
    };
    let address = node
        .address
        .as_deref()
        .or(node.canonical_address.as_deref());

    graph_module_matches(requested, address, module_name)
}

fn graph_module_matches(requested: &str, address: Option<&str>, module_name: &str) -> bool {
    if requested == module_name {
        return true;
    }

    address.is_some_and(|address| requested == format!("{address}::{module_name}"))
}

pub(crate) fn render_type_graph_text(graph: &MoveTypeGraph) -> String {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&MoveTypeGraphEdge>>::new();
    let mut unresolved = BTreeMap::<&str, Vec<&MoveUnresolvedType>>::new();

    for edge in &graph.edges {
        outgoing.entry(edge.source.as_str()).or_default().push(edge);
    }
    for ty in &graph.unresolved_types {
        unresolved.entry(ty.source.as_str()).or_default().push(ty);
    }

    let mut modules = BTreeMap::<String, Vec<&MoveTypeGraphNode>>::new();
    for node in &graph.nodes {
        modules
            .entry(type_graph_module_label(node))
            .or_default()
            .push(node);
    }

    let mut lines = vec![format!(
        "type graph nodes={} edges={} unresolved={}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.unresolved_types.len()
    )];

    for (module, mut module_nodes) in modules {
        module_nodes.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.name.cmp(&right.name))
        });
        lines.push(format!("|-- module {module}"));

        for node in module_nodes {
            let external = if node.is_external { " external" } else { "" };
            let abilities = if node.abilities.is_empty() {
                String::new()
            } else {
                format!(" abilities={}", node.abilities.join(","))
            };
            let attributes = if node.attributes.is_empty() {
                String::new()
            } else {
                format!(" attrs={}", node.attributes.join(","))
            };
            lines.push(format!(
                "|   |-- {} {} [{}{}{}{}]",
                node.kind,
                type_graph_node_label(node),
                node.source,
                external,
                abilities,
                attributes
            ));

            for edge in outgoing.get(node.id.as_str()).into_iter().flatten() {
                let target = nodes
                    .get(edge.target.as_str())
                    .map(|target| type_graph_node_qualified_label(target))
                    .unwrap_or_else(|| edge.target.clone());
                lines.push(format!(
                    "|   |   |-- {} -> {}",
                    type_graph_edge_label(edge),
                    target
                ));
            }

            for ty in unresolved.get(node.id.as_str()).into_iter().flatten() {
                lines.push(format!(
                    "|   |   |-- unresolved {} in {}: {}",
                    ty.raw_type, ty.context, ty.reason
                ));
            }
        }
    }

    lines.join("\n")
}

fn type_graph_module_label(node: &MoveTypeGraphNode) -> String {
    match (node.address.as_deref(), node.module_name.as_deref()) {
        (Some(address), Some(module)) => format!("{address}::{module}"),
        (None, Some(module)) => module.to_string(),
        _ if node.is_external => "<external>".to_string(),
        _ => "<unknown>".to_string(),
    }
}

fn type_graph_node_label(node: &MoveTypeGraphNode) -> String {
    if node.type_parameters.is_empty() {
        return node.name.clone();
    }

    let parameters = node
        .type_parameters
        .iter()
        .map(|parameter| {
            let mut label = String::new();
            if parameter.is_phantom {
                label.push_str("phantom ");
            }
            label.push_str(&parameter.name);
            if !parameter.abilities.is_empty() {
                label.push_str(": ");
                label.push_str(&parameter.abilities.join("+"));
            }
            label
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{}<{parameters}>", node.name)
}

fn type_graph_node_qualified_label(node: &MoveTypeGraphNode) -> String {
    if node.qualified_name.is_empty() {
        type_graph_node_label(node)
    } else {
        node.qualified_name.clone()
    }
}

fn type_graph_edge_label(edge: &MoveTypeGraphEdge) -> String {
    let mut details = Vec::new();

    if let Some(field_name) = &edge.field_name {
        details.push(format!("field={field_name}"));
    }
    if let Some(variant_name) = &edge.variant_name {
        details.push(format!("variant={variant_name}"));
    }
    if let Some(function_name) = &edge.function_name {
        details.push(format!("function={function_name}"));
    }
    if let Some(parameter_name) = &edge.parameter_name {
        details.push(format!("param={parameter_name}"));
    }
    if let Some(index) = edge.type_argument_index {
        details.push(format!("arg={index}"));
    }
    if let Some(type_expression) = &edge.type_expression {
        details.push(format!("type={type_expression}"));
    }
    if edge.is_reference {
        details.push("ref".to_string());
    }
    if edge.is_mutable {
        details.push("mut".to_string());
    }
    if edge.confidence != "high" {
        details.push(format!("confidence={}", edge.confidence));
    }

    if details.is_empty() {
        edge.relationship.clone()
    } else {
        format!("{} {}", edge.relationship, details.join(" "))
    }
}
