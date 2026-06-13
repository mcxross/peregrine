use crate::{
    MoveCallGraph, MoveSourceSpan, MoveStateAccessGraph, MoveTypeGraph, data_flow::build_data_flow,
    discover_move_project_model, discover_move_state_access_graph_for_function,
};
use peregrine_analysis::{
    AnalysisDiagnostic, AnalysisError, AnalysisFuture, AnalysisLimits, AnalysisOptions,
    AnalysisStage, ArtifactBundle, ChainId, GraphBuilder, GraphEdge, GraphKind, GraphNode,
    PluginDescriptor, PluginOrigin, PluginStage, PropertyGraph, SourceSpan,
};
use peregrine_sui_bytecode::load_package_bytecode;
use serde_json::json;
use std::{collections::BTreeMap, path::Path};

const PLUGIN_ID: &str = "peregrine.sui.move-graph";

#[derive(Default)]
pub struct SuiMoveGraphBuilder;

impl GraphBuilder for SuiMoveGraphBuilder {
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: PLUGIN_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            chain: ChainId::new("sui"),
            stage: PluginStage::GraphBuilder,
            capabilities: vec![
                GraphKind::DEPENDENCY.to_string(),
                GraphKind::CALL.to_string(),
                GraphKind::CONTROL_FLOW.to_string(),
                GraphKind::DATA_FLOW.to_string(),
                GraphKind::TYPE.to_string(),
                GraphKind::STATE_ACCESS.to_string(),
            ],
            origin: PluginOrigin::BuiltIn,
            priority: 100,
        }
    }

    fn supported_graphs(&self) -> Vec<GraphKind> {
        [
            GraphKind::DEPENDENCY,
            GraphKind::CALL,
            GraphKind::CONTROL_FLOW,
            GraphKind::DATA_FLOW,
            GraphKind::TYPE,
            GraphKind::STATE_ACCESS,
        ]
        .into_iter()
        .map(GraphKind::new)
        .collect()
    }

    fn build<'a>(
        &'a self,
        artifacts: &'a ArtifactBundle,
        requested: &'a [GraphKind],
        options: &'a AnalysisOptions,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, Vec<PropertyGraph>> {
        Box::pin(async move {
            let root = artifacts.package_root.as_ref().ok_or_else(|| {
                AnalysisError::new(
                    "package_not_materialized",
                    "Sui graph construction requires a locally materialized package",
                )
            })?;
            let model = discover_move_project_model(root);
            let mut graphs = Vec::new();

            if requested_kind(requested, GraphKind::DEPENDENCY) {
                graphs.push(dependency_graph(&model.dependency_graph));
            }
            if requested_kind(requested, GraphKind::CALL) {
                graphs.push(call_graph(&model.call_graph));
            }
            if requested_kind(requested, GraphKind::TYPE) {
                graphs.push(type_graph(&model.type_graph));
            }
            if requested_kind(requested, GraphKind::STATE_ACCESS) {
                let focused = options
                    .get("moduleName")
                    .and_then(serde_json::Value::as_str)
                    .zip(
                        options
                            .get("functionName")
                            .and_then(serde_json::Value::as_str),
                    )
                    .map(|(module_name, function_name)| {
                        discover_move_state_access_graph_for_function(
                            root,
                            options
                                .get("packagePath")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("."),
                            options
                                .get("address")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string),
                            module_name,
                            function_name,
                        )
                    });
                graphs.push(state_access_graph(
                    focused.as_ref().unwrap_or(&model.state_access_graph),
                ));
            }
            if requested_kind(requested, GraphKind::CONTROL_FLOW)
                || requested_kind(requested, GraphKind::DATA_FLOW)
            {
                let (control_flow, data_flow) = bytecode_graphs(root, artifacts);
                if requested_kind(requested, GraphKind::CONTROL_FLOW) {
                    graphs.push(control_flow);
                }
                if requested_kind(requested, GraphKind::DATA_FLOW) {
                    graphs.push(data_flow);
                }
            }
            Ok(graphs)
        })
    }
}

fn requested_kind(requested: &[GraphKind], kind: &str) -> bool {
    requested.iter().any(|requested| requested.0 == kind)
}

fn dependency_graph(source: &crate::PackageDependencyGraph) -> PropertyGraph {
    PropertyGraph {
        kind: GraphKind::new(GraphKind::DEPENDENCY),
        nodes: source
            .nodes
            .iter()
            .map(|node| GraphNode {
                id: node.id.clone(),
                kind: "package".to_string(),
                label: node.id.clone(),
                span: None,
                metadata: json!({
                    "address": node.address,
                    "moduleCount": node.module_count,
                    "publicFunctionCount": node.public_function_count,
                    "entryFunctionCount": node.entry_function_count,
                    "isRoot": node.is_root,
                }),
            })
            .collect(),
        edges: source
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| GraphEdge {
                id: format!("dependency:{index}:{}:{}", edge.source, edge.target),
                from: edge.source.clone(),
                to: edge.target.clone(),
                kind: edge.dependency_kind.clone(),
                spans: Vec::new(),
                evidence: Vec::new(),
                metadata: json!({"dependencyCount": edge.dependency_count}),
            })
            .collect(),
        diagnostics: Vec::new(),
        metadata: json!({
            "root": source.root,
            "summaryPath": source.summary_path,
            "legacyGraph": source,
        }),
    }
}

fn call_graph(source: &MoveCallGraph) -> PropertyGraph {
    PropertyGraph {
        kind: GraphKind::new(GraphKind::CALL),
        nodes: source
            .nodes
            .iter()
            .map(|node| GraphNode {
                id: node.id.clone(),
                kind: "function".to_string(),
                label: node.qualified_name.clone(),
                span: node.span.as_ref().map(convert_span),
                metadata: json!({
                    "packageName": node.package_name,
                    "packagePath": node.package_path,
                    "address": node.address,
                    "moduleName": node.module_name,
                    "functionName": node.function_name,
                    "visibility": node.visibility,
                    "isEntry": node.is_entry,
                    "isTransactionCallable": node.is_transaction_callable,
                    "isExternal": node.is_external,
                    "source": node.source,
                }),
            })
            .collect(),
        edges: source
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| GraphEdge {
                id: format!("call:{index}:{}:{}", edge.source, edge.target),
                from: edge.source.clone(),
                to: edge.target.clone(),
                kind: edge.call_kind.clone(),
                spans: edge.source_spans.iter().map(convert_span).collect(),
                evidence: vec![edge.raw_target.clone()],
                metadata: json!({
                    "confidence": edge.confidence,
                    "callCount": edge.call_count,
                    "typeArguments": edge.type_arguments,
                    "isExternal": edge.is_external,
                    "isResolved": edge.is_resolved,
                }),
            })
            .collect(),
        diagnostics: unresolved_diagnostics(
            "unresolved_call",
            source
                .unresolved_calls
                .iter()
                .map(|call| format!("{}: {}", call.raw_target, call.reason)),
        ),
        metadata: json!({
            "unresolvedCount": source.unresolved_calls.len(),
            "legacyGraph": source,
        }),
    }
}

fn type_graph(source: &MoveTypeGraph) -> PropertyGraph {
    PropertyGraph {
        kind: GraphKind::new(GraphKind::TYPE),
        nodes: source
            .nodes
            .iter()
            .map(|node| GraphNode {
                id: node.id.clone(),
                kind: node.kind.clone(),
                label: node.qualified_name.clone(),
                span: node.span.as_ref().map(convert_span),
                metadata: serde_json::to_value(node).unwrap_or_else(|_| json!({})),
            })
            .collect(),
        edges: source
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| GraphEdge {
                id: format!("type:{index}:{}:{}", edge.source, edge.target),
                from: edge.source.clone(),
                to: edge.target.clone(),
                kind: edge.relationship.clone(),
                spans: edge.source_spans.iter().map(convert_span).collect(),
                evidence: edge.evidence.clone(),
                metadata: serde_json::to_value(edge).unwrap_or_else(|_| json!({})),
            })
            .collect(),
        diagnostics: unresolved_diagnostics(
            "unresolved_type",
            source
                .unresolved_types
                .iter()
                .map(|value| format!("{}: {}", value.raw_type, value.reason)),
        ),
        metadata: json!({
            "unresolvedCount": source.unresolved_types.len(),
            "legacyGraph": source,
        }),
    }
}

fn state_access_graph(source: &MoveStateAccessGraph) -> PropertyGraph {
    PropertyGraph {
        kind: GraphKind::new(GraphKind::STATE_ACCESS),
        nodes: source
            .nodes
            .iter()
            .map(|node| GraphNode {
                id: node.id.clone(),
                kind: node.kind.clone(),
                label: node.qualified_name.clone(),
                span: node.span.as_ref().map(convert_span),
                metadata: serde_json::to_value(node).unwrap_or_else(|_| json!({})),
            })
            .collect(),
        edges: source
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| GraphEdge {
                id: format!("state:{index}:{}:{}", edge.source, edge.target),
                from: edge.source.clone(),
                to: edge.target.clone(),
                kind: edge.access_kind.clone(),
                spans: edge.source_spans.iter().map(convert_span).collect(),
                evidence: edge.evidence.clone(),
                metadata: serde_json::to_value(edge).unwrap_or_else(|_| json!({})),
            })
            .collect(),
        diagnostics: unresolved_diagnostics(
            "unresolved_state_access",
            source
                .unresolved_accesses
                .iter()
                .map(|access| format!("{}: {}", access.raw_target, access.reason)),
        ),
        metadata: json!({
            "unresolvedCount": source.unresolved_accesses.len(),
            "legacyGraph": source,
        }),
    }
}

fn bytecode_graphs(
    project_root: &Path,
    artifacts: &ArtifactBundle,
) -> (PropertyGraph, PropertyGraph) {
    let mut control_nodes = BTreeMap::new();
    let mut control_edges = Vec::new();
    let mut data_nodes = BTreeMap::new();
    let mut data_edges = Vec::new();
    let mut diagnostics = Vec::new();

    for package in artifacts
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == "package")
    {
        let package_root = package
            .path
            .as_deref()
            .filter(|path| !path.is_empty())
            .map(|path| project_root.join(path))
            .unwrap_or_else(|| project_root.to_path_buf());
        let view = match load_package_bytecode(&package_root, &package.name) {
            Ok(view) => view,
            Err(message) => {
                diagnostics.push(AnalysisDiagnostic::unavailable(
                    AnalysisStage::Graph,
                    Some(PLUGIN_ID.to_string()),
                    "compiled_move_artifacts_unavailable",
                    format!("{}: {message}", package.name),
                ));
                continue;
            }
        };
        for module in view.modules.iter().filter(|module| !module.is_dependency) {
            for function in &module.functions {
                add_control_flow(
                    &package.name,
                    module,
                    function,
                    &mut control_nodes,
                    &mut control_edges,
                );
                let data = build_data_flow(&package.name, module, function);
                data_nodes.extend(data.nodes.into_iter().map(|node| (node.id.clone(), node)));
                data_edges.extend(data.edges);
            }
        }
    }

    let common_metadata = json!({"bytecodeBacked": true});
    (
        PropertyGraph {
            kind: GraphKind::new(GraphKind::CONTROL_FLOW),
            nodes: control_nodes.into_values().collect(),
            edges: control_edges,
            diagnostics: diagnostics.clone(),
            metadata: common_metadata.clone(),
        },
        PropertyGraph {
            kind: GraphKind::new(GraphKind::DATA_FLOW),
            nodes: data_nodes.into_values().collect(),
            edges: data_edges,
            diagnostics,
            metadata: common_metadata,
        },
    )
}

fn add_control_flow(
    package_name: &str,
    module: &peregrine_sui_bytecode::MoveBytecodeModuleView,
    function: &peregrine_sui_bytecode::MoveBytecodeFunctionView,
    nodes: &mut BTreeMap<String, GraphNode>,
    edges: &mut Vec<GraphEdge>,
) {
    let function_id = format!(
        "{package_name}::{}::{}::{}",
        module.address, module.name, function.name
    );
    for block in &function.control_flow.blocks {
        let id = format!("{function_id}:{}", block.id);
        nodes.insert(
            id.clone(),
            GraphNode {
                id,
                kind: "basicBlock".to_string(),
                label: block.label.clone(),
                span: None,
                metadata: json!({
                    "function": function_id,
                    "startOffset": block.start_offset,
                    "endOffset": block.end_offset,
                    "instructionOffsets": block.instruction_offsets,
                }),
            },
        );
    }
    for (index, edge) in function.control_flow.edges.iter().enumerate() {
        edges.push(GraphEdge {
            id: format!("{function_id}:cfg-edge:{index}"),
            from: format!("{function_id}:{}", edge.source),
            to: format!("{function_id}:{}", edge.target),
            kind: edge.kind.clone(),
            spans: Vec::new(),
            evidence: Vec::new(),
            metadata: json!({
                "sourceOffset": edge.source_offset,
                "targetOffset": edge.target_offset,
            }),
        });
    }
}

fn unresolved_diagnostics(
    code: &str,
    messages: impl IntoIterator<Item = String>,
) -> Vec<AnalysisDiagnostic> {
    messages
        .into_iter()
        .take(100)
        .map(|message| {
            AnalysisDiagnostic::unavailable(
                AnalysisStage::Graph,
                Some(PLUGIN_ID.to_string()),
                code,
                message,
            )
        })
        .collect()
}

fn convert_span(span: &MoveSourceSpan) -> SourceSpan {
    SourceSpan {
        file_path: span.file_path.clone(),
        start_line: span.start_line,
        end_line: span.end_line,
        start_byte: span.start_byte,
        end_byte: span.end_byte,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_analysis::{Artifact, ChainId};
    use serde_json::json;

    #[tokio::test]
    async fn builds_all_required_graph_kinds_from_compiled_sui_move() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../peregrine-sui-indexer/tests/fixtures/sui/assert_then_call");
        let artifacts = artifact_bundle(root, "assert_then_call");

        let graphs = SuiMoveGraphBuilder
            .build(
                &artifacts,
                &GraphKind::required(),
                &AnalysisOptions::new(),
                &AnalysisLimits::default(),
            )
            .await
            .expect("required Sui graphs");

        assert_eq!(
            graphs
                .iter()
                .map(|graph| graph.kind.clone())
                .collect::<Vec<_>>(),
            GraphKind::required()
        );
        for kind in [GraphKind::CONTROL_FLOW, GraphKind::DATA_FLOW] {
            let graph = graphs
                .iter()
                .find(|graph| graph.kind.0 == kind)
                .expect("bytecode graph");
            assert!(!graph.nodes.is_empty(), "{kind} graph should contain nodes");
            assert!(
                graph
                    .diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.code != "compiled_move_artifacts_unavailable")
            );
        }
    }

    #[test]
    fn missing_build_artifacts_are_reported_as_unavailable() {
        let root = tempfile::tempdir().expect("temporary package");
        let artifacts = artifact_bundle(root.path().to_path_buf(), "missing_build");

        let (control_flow, data_flow) = bytecode_graphs(root.path(), &artifacts);

        for graph in [control_flow, data_flow] {
            assert!(graph.nodes.is_empty());
            assert!(graph.edges.is_empty());
            assert!(
                graph
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "compiled_move_artifacts_unavailable")
            );
        }
    }

    fn artifact_bundle(root: impl Into<std::path::PathBuf>, package_name: &str) -> ArtifactBundle {
        let root = root.into();
        ArtifactBundle {
            chain: ChainId::new("sui"),
            target_id: format!("local:{package_name}"),
            package_root: Some(root),
            artifacts: vec![Artifact {
                id: format!("package:{package_name}"),
                kind: "package".to_string(),
                name: package_name.to_string(),
                path: Some(String::new()),
                metadata: json!({}),
            }],
            symbols: Vec::new(),
            evidence: Vec::new(),
            diagnostics: Vec::new(),
            metadata: json!({}),
        }
    }
}
