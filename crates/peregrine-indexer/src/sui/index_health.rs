use std::collections::HashSet;

use serde_json::{json, Value};

use crate::{
    core::{
        is_neutral_tag, stable_id, Diagnostic, DiagnosticSeverity, EdgeType, MaterializedStatus,
        OperationKind, SourcePrecision, SourceSpan,
    },
    incremental::{InvalidationPlan, PackageFingerprints},
    sui::model::ProgramIndex,
};

pub fn harden_program_index(
    program: &mut ProgramIndex,
    fingerprints: &PackageFingerprints,
    previous_fingerprints_present: bool,
    invalidation: &InvalidationPlan,
    full_mode_requested: bool,
) -> Value {
    let coverage = Coverage::from_program(program);
    let integrity = Integrity::from_program(program);
    let readiness = readiness(&coverage, full_mode_requested);
    let freshness = freshness(previous_fingerprints_present, invalidation);

    let health = json!({
        "schemaVersion": 1,
        "indexerVersion": env!("CARGO_PKG_VERSION"),
        "readiness": readiness,
        "freshness": freshness,
        "fullModeRequested": full_mode_requested,
        "fingerprints": fingerprints,
        "invalidation": invalidation,
        "coverage": coverage.to_json(),
        "integrity": integrity.to_json(),
    });

    merge_index_health(program, health.clone());
    add_health_diagnostics(
        program,
        &coverage,
        &integrity,
        full_mode_requested,
        &readiness,
    );
    health
}

#[derive(Clone, Debug)]
struct Coverage {
    summary_artifact_count: usize,
    root_card_count: usize,
    direct_dependency_card_count: usize,
    pointer_only_summary_count: usize,
    expanded_summary_count: usize,
    source_file_count: usize,
    hashed_source_file_count: usize,
    function_count: usize,
    function_file_span_count: usize,
    operation_count: usize,
    operation_exact_span_count: usize,
    operation_source_span_count: usize,
    call_operation_count: usize,
    call_edge_with_operation_count: usize,
    field_read_edge_count: usize,
    field_write_edge_count: usize,
    control_flow_edge_count: usize,
}

impl Coverage {
    fn from_program(program: &ProgramIndex) -> Self {
        let source_files = program
            .files
            .iter()
            .filter(|file| file.kind == "move_source")
            .collect::<Vec<_>>();
        Self {
            summary_artifact_count: program.summary_artifacts.len(),
            root_card_count: program
                .summary_artifacts
                .iter()
                .filter(|artifact| artifact.materialized_status == MaterializedStatus::RootCard)
                .count(),
            direct_dependency_card_count: program
                .summary_artifacts
                .iter()
                .filter(|artifact| {
                    artifact.materialized_status == MaterializedStatus::DirectDependencyCard
                })
                .count(),
            pointer_only_summary_count: program
                .summary_artifacts
                .iter()
                .filter(|artifact| artifact.materialized_status == MaterializedStatus::PointerOnly)
                .count(),
            expanded_summary_count: program
                .summary_artifacts
                .iter()
                .filter(|artifact| {
                    matches!(
                        artifact.materialized_status,
                        MaterializedStatus::ExpandedModule | MaterializedStatus::ExpandedSymbol
                    )
                })
                .count(),
            source_file_count: source_files.len(),
            hashed_source_file_count: source_files
                .iter()
                .filter(|file| file.content_hash.is_some())
                .count(),
            function_count: program.functions.len(),
            function_file_span_count: program
                .functions
                .iter()
                .filter(|function| function.source_span.file_id.is_some())
                .count(),
            operation_count: program.operations.len(),
            operation_exact_span_count: program
                .operations
                .iter()
                .filter(|operation| {
                    operation.source_span.precision == SourcePrecision::ExactExpression
                })
                .count(),
            operation_source_span_count: program
                .operations
                .iter()
                .filter(|operation| operation.source_span.precision != SourcePrecision::Unknown)
                .count(),
            call_operation_count: program
                .operations
                .iter()
                .filter(|operation| operation.kind == OperationKind::Call)
                .count(),
            call_edge_with_operation_count: program
                .edges
                .iter()
                .filter(|edge| edge.edge_type == EdgeType::Calls && edge.operation_id.is_some())
                .count(),
            field_read_edge_count: program
                .edges
                .iter()
                .filter(|edge| edge.edge_type == EdgeType::ReadsField)
                .count(),
            field_write_edge_count: program
                .edges
                .iter()
                .filter(|edge| edge.edge_type == EdgeType::WritesField)
                .count(),
            control_flow_edge_count: program
                .edges
                .iter()
                .filter(|edge| edge.edge_type == EdgeType::ControlFlow)
                .count(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "summaryArtifactCount": self.summary_artifact_count,
            "rootCardCount": self.root_card_count,
            "directDependencyCardCount": self.direct_dependency_card_count,
            "pointerOnlySummaryCount": self.pointer_only_summary_count,
            "expandedSummaryCount": self.expanded_summary_count,
            "sourceFileCount": self.source_file_count,
            "hashedSourceFileCount": self.hashed_source_file_count,
            "functionCount": self.function_count,
            "functionFileSpanCount": self.function_file_span_count,
            "operationCount": self.operation_count,
            "operationExactSpanCount": self.operation_exact_span_count,
            "operationSourceSpanCount": self.operation_source_span_count,
            "callOperationCount": self.call_operation_count,
            "callEdgeWithOperationCount": self.call_edge_with_operation_count,
            "fieldReadEdgeCount": self.field_read_edge_count,
            "fieldWriteEdgeCount": self.field_write_edge_count,
            "controlFlowEdgeCount": self.control_flow_edge_count,
        })
    }
}

#[derive(Clone, Debug)]
struct Integrity {
    dangling_operation_edge_count: usize,
    dangling_semantic_tag_target_count: usize,
    prohibited_tag_count: usize,
}

impl Integrity {
    fn from_program(program: &ProgramIndex) -> Self {
        let operation_ids = program
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<HashSet<_>>();
        let target_ids = program
            .functions
            .iter()
            .map(|function| function.id.as_str())
            .chain(program.types.iter().map(|type_def| type_def.id.as_str()))
            .chain(program.modules.iter().map(|module| module.id.as_str()))
            .chain(program.fields.iter().map(|field| field.id.as_str()))
            .chain(
                program
                    .operations
                    .iter()
                    .map(|operation| operation.id.as_str()),
            )
            .chain(
                program
                    .summary_artifacts
                    .iter()
                    .map(|artifact| artifact.id.as_str()),
            )
            .chain(std::iter::once(program.package.id.as_str()))
            .collect::<HashSet<_>>();

        Self {
            dangling_operation_edge_count: program
                .edges
                .iter()
                .filter_map(|edge| edge.operation_id.as_deref())
                .filter(|operation_id| !operation_ids.contains(operation_id))
                .count(),
            dangling_semantic_tag_target_count: program
                .semantic_tags
                .iter()
                .filter(|tag| !target_ids.contains(tag.target_id.as_str()))
                .count(),
            prohibited_tag_count: program
                .semantic_tags
                .iter()
                .filter(|tag| !is_neutral_tag(&tag.tag))
                .count(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "danglingOperationEdgeCount": self.dangling_operation_edge_count,
            "danglingSemanticTagTargetCount": self.dangling_semantic_tag_target_count,
            "prohibitedTagCount": self.prohibited_tag_count,
        })
    }
}

fn readiness(coverage: &Coverage, full_mode_requested: bool) -> &'static str {
    if coverage.summary_artifact_count == 0 {
        "missing_summary_artifacts"
    } else if full_mode_requested && coverage.operation_count > 0 {
        "compiler_backed"
    } else if coverage.summary_artifact_count > 0 {
        "summary_pointer"
    } else {
        "partial"
    }
}

fn freshness(previous_fingerprints_present: bool, invalidation: &InvalidationPlan) -> &'static str {
    if invalidation.is_clean() {
        "fresh"
    } else if previous_fingerprints_present {
        "changed"
    } else {
        "first_index"
    }
}

fn merge_index_health(program: &mut ProgramIndex, health: Value) {
    let mut metadata = program
        .package
        .metadata_json
        .take()
        .unwrap_or_else(|| json!({}));
    if !metadata.is_object() {
        metadata = json!({ "previousMetadata": metadata });
    }
    if let Some(object) = metadata.as_object_mut() {
        object.insert("index_health".to_string(), health);
    }
    program.package.metadata_json = Some(metadata);
}

fn add_health_diagnostics(
    program: &mut ProgramIndex,
    coverage: &Coverage,
    integrity: &Integrity,
    full_mode_requested: bool,
    readiness: &str,
) {
    if coverage.summary_artifact_count == 0 {
        push_health_diagnostic(
            program,
            "missing_summary_artifacts",
            DiagnosticSeverity::Warning,
            "No package summary artifacts were discovered; package context is limited to available source/build facts.",
            json!({ "readiness": readiness }),
        );
    }
    if full_mode_requested && coverage.function_count > 0 && coverage.operation_count == 0 {
        push_health_diagnostic(
            program,
            "body_operations_unavailable",
            DiagnosticSeverity::Info,
            "Compiler-backed operation index is unavailable; function bodies remain summary/source-span only.",
            json!({
                "functionCount": coverage.function_count,
                "operationCount": coverage.operation_count,
            }),
        );
    }
    if coverage.operation_count > 0 && coverage.operation_exact_span_count == 0 {
        push_health_diagnostic(
            program,
            "exact_operation_spans_unavailable",
            DiagnosticSeverity::Info,
            "Exact source-map operation spans are unavailable; operation spans use fallback precision.",
            json!({
                "operationCount": coverage.operation_count,
                "operationSourceSpanCount": coverage.operation_source_span_count,
            }),
        );
    }
    if coverage.call_operation_count != coverage.call_edge_with_operation_count {
        push_health_diagnostic(
            program,
            "call_edge_parity_mismatch",
            DiagnosticSeverity::Warning,
            "CALLS edge count does not match bytecode call operation count.",
            json!({
                "callOperationCount": coverage.call_operation_count,
                "callEdgeWithOperationCount": coverage.call_edge_with_operation_count,
            }),
        );
    }
    if integrity.dangling_operation_edge_count > 0 {
        push_health_diagnostic(
            program,
            "dangling_operation_edges",
            DiagnosticSeverity::Error,
            "Edges reference operations that are not present in the operation table.",
            json!({
                "danglingOperationEdgeCount": integrity.dangling_operation_edge_count,
            }),
        );
    }
    if integrity.dangling_semantic_tag_target_count > 0 {
        push_health_diagnostic(
            program,
            "dangling_semantic_tag_targets",
            DiagnosticSeverity::Error,
            "Semantic tags reference targets that are not present in the indexed entity tables.",
            json!({
                "danglingSemanticTagTargetCount": integrity.dangling_semantic_tag_target_count,
            }),
        );
    }
    if integrity.prohibited_tag_count > 0 {
        push_health_diagnostic(
            program,
            "non_neutral_semantic_tags",
            DiagnosticSeverity::Error,
            "Non-neutral semantic tags were detected and should be removed from the index.",
            json!({
                "prohibitedTagCount": integrity.prohibited_tag_count,
            }),
        );
    }
}

fn push_health_diagnostic(
    program: &mut ProgramIndex,
    code: &str,
    severity: DiagnosticSeverity,
    message: &str,
    metadata: Value,
) {
    let id = stable_id("diagnostic", [&program.package.id, "index_health", code]);
    if program
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == id)
    {
        return;
    }
    program.diagnostics.push(Diagnostic {
        id,
        package_id: program.package.id.clone(),
        severity,
        source: "index_health".to_string(),
        message: message.to_string(),
        source_span: SourceSpan::unknown(),
        metadata_json: Some(metadata),
    });
}
