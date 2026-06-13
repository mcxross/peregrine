use super::super::{
    call_graph::{
        external_function_id, finish_call_graph, function_id, unresolved_call_id, MoveCallGraph,
        MoveCallGraphEdge, MoveCallGraphNode, MoveSourceSpan, MoveUnresolvedCall,
    },
    state_access_graph::{
        finish_state_access_graph, state_field_id, MoveStateAccessGraph, MoveStateAccessGraphEdge,
        MoveStateAccessGraphNode, MoveUnresolvedStateAccess,
    },
    type_graph::{
        builtin_type_id, external_type_id, finish_type_graph, type_id, type_parameter_id,
        MoveTypeGraph, MoveTypeGraphEdge, MoveTypeGraphNode, MoveTypeParameter, MoveUnresolvedType,
    },
};
use super::{
    source_spans::{source_for_range, source_span, summary_span},
    summaries::{
        read_address_mapping, read_summary_modules, resolve_summary_location, summary_fields,
    },
};
use move_command_line_common::files::FileHash;
use move_compiler::{
    editions::Flavor,
    parser::{
        ast::{
            Ability_, Attribute_, Attributes, Bind, Bind_, Definition, EnumDefinition, Exp, Exp_,
            Function, FunctionBody_, LeadingNameAccess, LeadingNameAccess_, MatchPattern,
            MatchPattern_, ModuleDefinition, ModuleMember, ModuleUse, NameAccessChain,
            NameAccessChain_, Sequence, SequenceItem_, StructDefinition, StructFields, Type, Type_,
            Use, UseDecl, VariantFields, Visibility,
        },
        syntax::parse_file_string,
    },
    shared::{CompilationEnv, Name, PackageConfig},
    Flags,
};
use peregrine_sui_move_model::{relative_path, MovePackageModel};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const BUILTIN_TYPES: &[&str] = &[
    "address", "bool", "signer", "u8", "u16", "u32", "u64", "u128", "u256", "vector",
];

const COPY_DROP_STORE_ABILITIES: &[&str] = &["copy", "drop", "store"];

pub(crate) fn build_move_graphs(
    root: &Path,
    packages: &[MovePackageModel],
) -> (MoveCallGraph, MoveTypeGraph, MoveStateAccessGraph) {
    let modules = parse_source_modules(root, packages);
    let mut builder = GraphBuilder::default();

    builder.index_source_modules(&modules);
    builder.collect_source_relationships(&modules);
    builder.enrich_from_summaries(root, packages);
    builder.finish()
}

pub(crate) struct MoveStateAccessGraphTarget {
    pub package_path: String,
    pub address: Option<String>,
    pub module_name: String,
    pub function_name: String,
    pub max_call_depth: usize,
}

pub(crate) fn build_move_state_access_graph(
    root: &Path,
    packages: &[MovePackageModel],
    target: MoveStateAccessGraphTarget,
) -> MoveStateAccessGraph {
    let modules = parse_source_modules(root, packages);
    let mut builder = GraphBuilder::default();

    builder.index_source_modules(&modules);
    builder.collect_source_type_relationships(&modules);
    builder.collect_reachable_function_state_relationships(&modules, &target);
    builder.finish_state_access_graph()
}

#[derive(Clone)]
struct SourceModule {
    package_name: String,
    package_path: String,
    address: Option<String>,
    name: String,
    file_path: String,
    source: Arc<str>,
    module: ModuleDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ModuleRef {
    address: Option<String>,
    module: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct MemberRef {
    address: Option<String>,
    module: String,
    member: String,
}

#[derive(Clone, Debug, Default)]
struct AliasScope {
    module_aliases: BTreeMap<String, ModuleRef>,
    member_aliases: BTreeMap<String, MemberRef>,
    method_aliases: BTreeMap<String, MemberRef>,
}

#[derive(Clone)]
struct ModuleContext {
    package_name: String,
    package_path: String,
    address: Option<String>,
    module_name: String,
    file_path: String,
    source: Arc<str>,
    aliases: AliasScope,
}

#[derive(Clone)]
struct FunctionContext {
    module: ModuleContext,
    function_id: String,
    function_name: String,
    type_parameters: BTreeMap<String, String>,
    local_state_types: BTreeMap<String, String>,
}

#[derive(Clone)]
struct TypeContext {
    owner_id: String,
    owner_name: Option<String>,
    module: ModuleContext,
    type_parameters: BTreeMap<String, String>,
}

#[derive(Clone)]
struct TypeUse {
    id: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct CallEdgeKey {
    source: String,
    target: String,
    call_kind: String,
    raw_target: String,
    type_arguments: Vec<String>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct TypeEdgeKey {
    source: String,
    target: String,
    relationship: String,
    field_name: Option<String>,
    variant_name: Option<String>,
    function_name: Option<String>,
    parameter_name: Option<String>,
    type_argument_index: Option<usize>,
    is_mutable: bool,
    is_reference: bool,
    declaring_type_id: Option<String>,
    declaring_field_name: Option<String>,
    type_argument_name: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct UnresolvedCallKey {
    source: String,
    raw_target: String,
    call_kind: String,
    file_path: String,
    reason: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct UnresolvedTypeKey {
    source: String,
    raw_type: String,
    context: String,
    file_path: String,
    reason: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct StateAccessEdgeKey {
    source: String,
    target: String,
    access_kind: String,
    field_name: Option<String>,
    via_function: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct UnresolvedStateAccessKey {
    source: String,
    raw_target: String,
    access_kind: String,
    file_path: String,
    reason: String,
}

#[derive(Default)]
struct GraphBuilder {
    call_nodes: BTreeMap<String, MoveCallGraphNode>,
    call_edges: BTreeMap<CallEdgeKey, MoveCallGraphEdge>,
    unresolved_calls: BTreeMap<UnresolvedCallKey, MoveUnresolvedCall>,
    type_nodes: BTreeMap<String, MoveTypeGraphNode>,
    type_edges: BTreeMap<TypeEdgeKey, MoveTypeGraphEdge>,
    unresolved_types: BTreeMap<UnresolvedTypeKey, MoveUnresolvedType>,
    state_nodes: BTreeMap<String, MoveStateAccessGraphNode>,
    state_edges: BTreeMap<StateAccessEdgeKey, MoveStateAccessGraphEdge>,
    unresolved_state_accesses: BTreeMap<UnresolvedStateAccessKey, MoveUnresolvedStateAccess>,
    function_exact: BTreeMap<(Option<String>, String, String), String>,
    function_by_module_member: BTreeMap<(String, String), BTreeSet<String>>,
    type_exact: BTreeMap<(Option<String>, String, String), String>,
    type_by_module_member: BTreeMap<(String, String), BTreeSet<String>>,
    address_mapping: BTreeMap<String, String>,
}

