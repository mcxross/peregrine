use crate::{
    core::{
        Diagnostic, DiagnosticSeverity, Edge, EdgeType, SourcePrecision, SourceSpan, stable_id,
    },
    model::ProgramIndex,
};
use peregrine_analysis::{
    AnalysisDiagnostic, ArtifactBundle, DiagnosticSeverity as AnalysisSeverity, GraphKind,
    PropertyGraph, SourceSpan as AnalysisSpan,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

pub(crate) fn merge_normalized_analysis(
    program: &mut ProgramIndex,
    artifacts: &ArtifactBundle,
    graphs: &[PropertyGraph],
) -> Result<(), String> {
    if artifacts.chain.as_str() != "sui" {
        return Err(format!(
            "Sui indexer cannot persist `{}` analysis artifacts",
            artifacts.chain.as_str()
        ));
    }

    let metadata = program
        .package
        .metadata_json
        .get_or_insert_with(|| Value::Object(Map::new()));
    let Some(metadata) = metadata.as_object_mut() else {
        return Err("package metadata must be a JSON object".to_string());
    };
    metadata.insert(
        "normalizedAnalysis".to_string(),
        json!({
            "targetId": artifacts.target_id,
            "artifactCount": artifacts.artifacts.len(),
            "symbolCount": artifacts.symbols.len(),
            "evidenceCount": artifacts.evidence.len(),
            "graphSummaries": graphs.iter().map(|graph| json!({
                "kind": graph.kind,
                "nodeCount": graph.nodes.len(),
                "edgeCount": graph.edges.len(),
            })).collect::<Vec<_>>(),
        }),
    );

    let mut diagnostic_ids = program
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id.clone())
        .collect::<BTreeSet<_>>();
    for diagnostic in artifacts
        .diagnostics
        .iter()
        .chain(graphs.iter().flat_map(|graph| &graph.diagnostics))
    {
        let diagnostic = index_diagnostic(&program.package.id, diagnostic);
        if diagnostic_ids.insert(diagnostic.id.clone()) {
            program.diagnostics.push(diagnostic);
        }
    }

    let mut edge_ids = program
        .edges
        .iter()
        .map(|edge| edge.id.clone())
        .collect::<BTreeSet<_>>();
    for graph in graphs {
        for edge in &graph.edges {
            let id = stable_id(
                "analysis-edge",
                [
                    program.package.id.as_str(),
                    graph.kind.0.as_str(),
                    edge.id.as_str(),
                ],
            );
            if !edge_ids.insert(id.clone()) {
                continue;
            }
            program.edges.push(Edge {
                id,
                package_id: program.package.id.clone(),
                from_id: edge.from.clone(),
                to_id: edge.to.clone(),
                edge_type: edge_type(&graph.kind, &edge.kind),
                operation_id: None,
                source_span: edge
                    .spans
                    .first()
                    .map(index_span)
                    .unwrap_or_else(SourceSpan::unknown),
                metadata_json: Some(json!({
                    "analysisGraphKind": graph.kind,
                    "analysisEdgeKind": edge.kind,
                    "evidence": edge.evidence,
                    "metadata": edge.metadata,
                })),
            });
        }
    }
    Ok(())
}

fn index_diagnostic(package_id: &str, diagnostic: &AnalysisDiagnostic) -> Diagnostic {
    Diagnostic {
        id: stable_id(
            "analysis-diagnostic",
            [
                package_id,
                diagnostic.code.as_str(),
                diagnostic.message.as_str(),
            ],
        ),
        package_id: package_id.to_string(),
        severity: match diagnostic.severity {
            AnalysisSeverity::Info => DiagnosticSeverity::Info,
            AnalysisSeverity::Warning => DiagnosticSeverity::Warning,
            AnalysisSeverity::Error => DiagnosticSeverity::Error,
        },
        source: diagnostic
            .plugin_id
            .clone()
            .unwrap_or_else(|| format!("{:?}", diagnostic.stage).to_ascii_lowercase()),
        message: diagnostic.message.clone(),
        source_span: SourceSpan::unknown(),
        metadata_json: Some(json!({
            "code": diagnostic.code,
            "stage": diagnostic.stage,
        })),
    }
}

fn index_span(span: &AnalysisSpan) -> SourceSpan {
    SourceSpan {
        file_id: Some(span.file_path.clone()),
        summary_artifact_id: None,
        start_line: u32::try_from(span.start_line).ok(),
        start_col: None,
        end_line: u32::try_from(span.end_line).ok(),
        end_col: None,
        precision: SourcePrecision::ExactExpression,
    }
}

fn edge_type(graph_kind: &GraphKind, edge_kind: &str) -> EdgeType {
    match (graph_kind.0.as_str(), edge_kind) {
        (GraphKind::CALL, _) => EdgeType::Calls,
        (GraphKind::CONTROL_FLOW, _) => EdgeType::ControlFlow,
        (GraphKind::DATA_FLOW, _) => EdgeType::DataFlow,
        (GraphKind::DEPENDENCY, _) => EdgeType::DependsOnModule,
        (GraphKind::TYPE, _) => EdgeType::ReferencesType,
        (GraphKind::STATE_ACCESS, "read" | "reads") => EdgeType::ReadsField,
        (GraphKind::STATE_ACCESS, "write" | "writes") => EdgeType::WritesField,
        (GraphKind::STATE_ACCESS, "borrow" | "borrows") => EdgeType::BorrowsField,
        (GraphKind::STATE_ACCESS, "borrowMut" | "borrowsMut") => EdgeType::BorrowsFieldMut,
        _ => EdgeType::AnalysisRelation,
    }
}
