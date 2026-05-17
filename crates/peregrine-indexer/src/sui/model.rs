use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::{
    AddressMapping, BasicBlock, ContextBudget, ContextLevel, Diagnostic, Edge, FieldInfo,
    FunctionInfo, FunctionParameter, LocalInfo, ModuleInfo, Operation, PackageInfo, SemanticTag,
    SummaryArtifact, TypeDef,
};

#[derive(Clone, Debug)]
pub struct LoadedPackage {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub package_name: String,
    pub manifest_hash: String,
}

#[derive(Clone, Debug)]
pub struct CompiledPackage {
    pub loaded: LoadedPackage,
    pub build_root: Option<PathBuf>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct SummaryArtifacts {
    pub package: LoadedPackage,
    pub summary_root: Option<PathBuf>,
    pub address_mapping_path: Option<PathBuf>,
    pub root_metadata_path: Option<PathBuf>,
    pub summary_files: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct SummaryPointerIndex {
    pub program_index: ProgramIndex,
    pub summary_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct SummaryMaterializationRequest {
    pub db_path: PathBuf,
    pub package_alias: String,
    pub module_name: String,
    pub symbol_name: Option<String>,
    pub budget: ContextBudget,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterializedSummaryContext {
    pub card: ModuleSummaryCard,
}

#[derive(Clone, Debug, Default)]
pub struct ExtractionContext;

#[derive(Clone, Debug)]
pub struct ProgramIndex {
    pub package: PackageInfo,
    pub files: Vec<SourceFileRecord>,
    pub summary_artifacts: Vec<SummaryArtifact>,
    pub address_mappings: Vec<AddressMapping>,
    pub modules: Vec<ModuleInfo>,
    pub dependencies: Vec<DependencyRecord>,
    pub types: Vec<TypeDef>,
    pub fields: Vec<FieldInfo>,
    pub functions: Vec<FunctionInfo>,
    pub locals: Vec<LocalInfo>,
    pub basic_blocks: Vec<BasicBlock>,
    pub operations: Vec<Operation>,
    pub edges: Vec<Edge>,
    pub semantic_tags: Vec<SemanticTag>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileRecord {
    pub id: String,
    pub path: String,
    pub content_hash: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyRecord {
    pub id: String,
    pub package_id: String,
    pub source_package_alias: String,
    pub source_module: String,
    pub target_package_alias: String,
    pub target_module: String,
    pub dependency_kind: String,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexReport {
    pub run_id: String,
    pub package_id: String,
    pub package_name: String,
    pub db_path: String,
    pub status: String,
    pub index_health: Option<serde_json::Value>,
    pub summary_artifact_count: usize,
    pub module_count: usize,
    pub function_count: usize,
    pub type_count: usize,
    pub operation_count: usize,
    pub diagnostic_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageOverview {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub status: String,
    pub indexed_at: i64,
    pub index_health: Option<serde_json::Value>,
    pub modules: usize,
    pub functions: usize,
    pub types: usize,
    pub summary_artifacts: usize,
    pub pointer_only_summaries: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleContext {
    pub module: ModuleInfo,
    pub functions: Vec<SymbolResult>,
    pub types: Vec<SymbolResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeContext {
    pub type_def: TypeDef,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionContext {
    pub card: FunctionSymbolCard,
    pub outline: FunctionOutline,
    pub evidence: FunctionEvidenceSummary,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
    pub reachable_callees: Vec<String>,
    pub field_reads: Vec<String>,
    pub field_writes: Vec<String>,
    pub related_types: Vec<RelatedTypeCard>,
    pub operation_histogram: Vec<OperationHistogramEntry>,
    pub operations: Vec<Operation>,
    pub source_excerpts: Vec<SourceExcerpt>,
    pub diagnostics: Vec<Diagnostic>,
    pub estimated_tokens: usize,
    pub budget_tokens: usize,
    pub trimmed: bool,
    pub trim_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSymbolCard {
    pub id: String,
    pub kind: String,
    pub full_name: String,
    pub signature: String,
    pub visibility: String,
    pub is_entry: bool,
    pub source_span: crate::core::SourceSpan,
    pub top_tags: Vec<String>,
}

impl FunctionSymbolCard {
    pub fn from_function(function: &FunctionInfo, tags: Vec<SemanticTag>) -> Self {
        let params = function
            .parameters
            .iter()
            .map(|parameter| match &parameter.name {
                Some(name) => format!("{name}: {}", parameter.type_name),
                None => parameter.type_name.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let returns = if function.returns.is_empty() {
            String::new()
        } else {
            format!(": {}", function.returns.join(", "))
        };

        Self {
            id: function.id.clone(),
            kind: "function".to_string(),
            full_name: function.full_name.clone(),
            signature: format!("{}({params}){returns}", function.name),
            visibility: format!("{:?}", function.visibility),
            is_entry: function.is_entry,
            source_span: function.source_span.clone(),
            top_tags: tags.into_iter().map(|tag| tag.tag).take(8).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionOutline {
    pub params: Vec<FunctionParameter>,
    pub returns: Vec<String>,
    pub direct_calls: Vec<String>,
    pub operation_count: usize,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionEvidenceSummary {
    pub body_indexed: bool,
    pub operation_count: usize,
    pub exact_operation_spans: usize,
    pub source_mapped_operations: usize,
    pub call_operation_count: usize,
    pub call_edge_count: usize,
    pub field_read_count: usize,
    pub field_write_count: usize,
    pub source_precision: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationHistogramEntry {
    pub kind: String,
    pub count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedTypeCard {
    pub id: String,
    pub full_name: String,
    pub kind: String,
    pub abilities: Vec<String>,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceExcerpt {
    pub file_id: String,
    pub start_line: u32,
    pub end_line: u32,
    pub precision: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolResult {
    pub id: String,
    pub kind: String,
    pub full_name: String,
    pub visibility: String,
    pub is_entry: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphView {
    pub nodes: Vec<String>,
    pub edges: Vec<(String, String)>,
    pub trimmed: bool,
    pub trim_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPack {
    pub target_id: String,
    pub level: ContextLevel,
    pub sections: Vec<String>,
    pub estimated_tokens: usize,
    pub budget_tokens: usize,
    pub trimmed: bool,
    pub trim_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSummaryCard {
    pub artifact_id: String,
    pub package_alias: String,
    pub module_name: String,
    pub summary_path: String,
    pub content_hash: String,
    pub role: String,
    pub materialized_status: String,
    pub card: Option<serde_json::Value>,
    pub estimated_tokens: usize,
    pub budget_tokens: usize,
    pub trimmed: bool,
    pub trim_reasons: Vec<String>,
}
