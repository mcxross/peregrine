use crate::{
    core::MaterializedStatus,
    model::{IndexLayerSummary, ProgramIndex},
};

#[derive(Clone, Debug, Default)]
pub struct IndexLayerCounts {
    pub file_count: usize,
    pub summary_artifact_count: usize,
    pub root_card_count: usize,
    pub direct_dependency_card_count: usize,
    pub expanded_summary_count: usize,
    pub module_count: usize,
    pub function_count: usize,
    pub type_count: usize,
    pub field_count: usize,
    pub operation_count: usize,
    pub edge_count: usize,
    pub diagnostic_count: usize,
    pub context_pack_count: usize,
}

pub fn summarize_program_layers(program: &ProgramIndex) -> Vec<IndexLayerSummary> {
    summarize_index_layers(IndexLayerCounts {
        file_count: program.files.len(),
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
        module_count: program.modules.len(),
        function_count: program.functions.len(),
        type_count: program.types.len(),
        field_count: program.fields.len()
            + program
                .types
                .iter()
                .map(|type_def| type_def.fields.len())
                .sum::<usize>(),
        operation_count: program.operations.len(),
        edge_count: program.edges.len(),
        diagnostic_count: program.diagnostics.len(),
        context_pack_count: 0,
    })
}

pub fn summarize_index_layers(counts: IndexLayerCounts) -> Vec<IndexLayerSummary> {
    vec![
        layer(
            "artifact_pointer",
            "Track compiler/source artifact paths, hashes, roles, and materialization state.",
            ready_status(counts.file_count + counts.summary_artifact_count),
            counts.file_count + counts.summary_artifact_count,
            &["files", "summary_artifacts", "address_mappings"],
            &[
                "Move.toml",
                "package_summaries",
                "address_mapping",
                "root_package_metadata",
            ],
            &["get_package_overview", "get_summary_artifact_pointer"],
            "Proves which compiler artifacts backed the map without duplicating raw artifacts.",
        ),
        layer(
            "symbol_card",
            "Expose compact root module/type/function cards for model navigation.",
            ready_status(counts.module_count + counts.function_count + counts.type_count),
            counts.module_count + counts.function_count + counts.type_count,
            &["modules", "types", "functions", "semantic_tags"],
            &["package_summaries", "sources"],
            &["search_symbols", "get_module_context", "get_type_context"],
            "Gives agents a sparse symbol map before expanding bodies or dependencies.",
        ),
        layer(
            "summary_card",
            "Materialize root summaries and directly relevant dependency summaries into compact cards.",
            summary_card_status(&counts),
            counts.root_card_count
                + counts.direct_dependency_card_count
                + counts.expanded_summary_count,
            &["summary_artifacts", "modules", "types", "functions"],
            &["package_summaries"],
            &["materialize_summary_module", "materialize_summary_symbol"],
            "Keeps dependency context pointer/card-only unless a query requires expansion.",
        ),
        layer(
            "function_signature",
            "Index visibility, entry flag, type parameters, parameters, returns, acquires, docs, and attributes.",
            ready_status(counts.function_count),
            counts.function_count,
            &["functions", "semantic_tags"],
            &["package_summaries", "sources"],
            &["get_public_entry_functions", "get_function_context"],
            "Lets agents find callable surfaces and function shapes without reading bodies.",
        ),
        layer(
            "type_field",
            "Index struct/enum shape, abilities, fields, and type-level retrieval tags.",
            ready_status(counts.type_count + counts.field_count),
            counts.type_count + counts.field_count,
            &["types", "fields", "edges", "semantic_tags"],
            &["package_summaries", "bytecode_modules"],
            &[
                "get_type_context",
                "get_function_field_reads",
                "get_function_field_writes",
            ],
            "Supports object, capability, state, and field-access reasoning from neutral facts.",
        ),
        layer(
            "operation",
            "Index compiler-backed normalized function operations when bytecode/source-map evidence exists.",
            operation_status(counts.operation_count),
            counts.operation_count,
            &["operations", "basic_blocks", "semantic_tags"],
            &["bytecode_modules", "debug_info", "source_maps"],
            &[
                "get_function_operations",
                "get_operations_by_tag",
                "get_function_body",
            ],
            "Provides ordered body evidence without claiming whether behavior is secure.",
        ),
        layer(
            "graph_edge",
            "Index containment, calls, type references, field accesses, and control-flow edges.",
            ready_status(counts.edge_count),
            counts.edge_count,
            &["edges"],
            &["package_summaries", "bytecode_modules", "source_maps"],
            &[
                "get_function_callees",
                "get_reachable_callees",
                "get_call_graph",
            ],
            "Connects functions, types, operations, and dependencies for bounded graph expansion.",
        ),
        layer(
            "diagnostic",
            "Index compiler, parsing, source-map, and index-health diagnostics.",
            diagnostic_status(counts.diagnostic_count),
            counts.diagnostic_count,
            &["diagnostics"],
            &["compiler_output", "summary_parser", "index_health"],
            &["get_diagnostics", "get_package_overview"],
            "Surfaces degraded or missing evidence instead of hiding uncertainty.",
        ),
        layer(
            "context_pack_cache",
            "Cache bounded model context packs by target, level, and budget fingerprint.",
            context_cache_status(counts.context_pack_count),
            counts.context_pack_count,
            &["chunks"],
            &["normalized_index", "context_budget"],
            &["get_context_pack"],
            "Reuses deterministic compact context while avoiding raw source or raw artifact dumps by default.",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn layer(
    name: &str,
    purpose: &str,
    status: &str,
    fact_count: usize,
    backing_tables: &[&str],
    freshness_inputs: &[&str],
    primary_queries: &[&str],
    security_context_value: &str,
) -> IndexLayerSummary {
    IndexLayerSummary {
        name: name.to_string(),
        purpose: purpose.to_string(),
        status: status.to_string(),
        fact_count,
        backing_tables: backing_tables
            .iter()
            .map(|value| value.to_string())
            .collect(),
        freshness_inputs: freshness_inputs
            .iter()
            .map(|value| value.to_string())
            .collect(),
        primary_queries: primary_queries
            .iter()
            .map(|value| value.to_string())
            .collect(),
        security_context_value: security_context_value.to_string(),
    }
}

fn ready_status(count: usize) -> &'static str {
    if count > 0 { "ready" } else { "empty" }
}

fn summary_card_status(counts: &IndexLayerCounts) -> &'static str {
    if counts.root_card_count + counts.direct_dependency_card_count + counts.expanded_summary_count
        > 0
    {
        "ready"
    } else if counts.summary_artifact_count > 0 {
        "pointer_only"
    } else {
        "empty"
    }
}

fn operation_status(count: usize) -> &'static str {
    if count > 0 { "ready" } else { "summary_only" }
}

fn diagnostic_status(count: usize) -> &'static str {
    if count > 0 {
        "has_diagnostics"
    } else {
        "empty"
    }
}

fn context_cache_status(count: usize) -> &'static str {
    if count > 0 { "ready" } else { "empty_cache" }
}
