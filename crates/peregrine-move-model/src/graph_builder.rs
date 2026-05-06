use super::{relative_path, MovePackageModel};
use crate::{
    call_graph::{
        external_function_id, finish_call_graph, function_id, unresolved_call_id, MoveCallGraph,
        MoveCallGraphEdge, MoveCallGraphNode, MoveSourceSpan, MoveUnresolvedCall,
    },
    type_graph::{
        builtin_type_id, external_type_id, finish_type_graph, type_id, type_parameter_id,
        MoveTypeGraph, MoveTypeGraphEdge, MoveTypeGraphNode, MoveTypeParameter, MoveUnresolvedType,
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
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
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
) -> (MoveCallGraph, MoveTypeGraph) {
    let modules = parse_source_modules(root, packages);
    let mut builder = GraphBuilder::default();

    builder.index_source_modules(&modules);
    builder.collect_source_relationships(&modules);
    builder.enrich_from_summaries(root, packages);
    builder.finish()
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

#[derive(Default)]
struct GraphBuilder {
    call_nodes: BTreeMap<String, MoveCallGraphNode>,
    call_edges: BTreeMap<CallEdgeKey, MoveCallGraphEdge>,
    unresolved_calls: BTreeMap<UnresolvedCallKey, MoveUnresolvedCall>,
    type_nodes: BTreeMap<String, MoveTypeGraphNode>,
    type_edges: BTreeMap<TypeEdgeKey, MoveTypeGraphEdge>,
    unresolved_types: BTreeMap<UnresolvedTypeKey, MoveUnresolvedType>,
    function_exact: BTreeMap<(Option<String>, String, String), String>,
    function_by_module_member: BTreeMap<(String, String), BTreeSet<String>>,
    type_exact: BTreeMap<(Option<String>, String, String), String>,
    type_by_module_member: BTreeMap<(String, String), BTreeSet<String>>,
    address_mapping: BTreeMap<String, String>,
}

impl GraphBuilder {
    fn index_source_modules(&mut self, modules: &[SourceModule]) {
        for module in modules {
            let module_context = module_context(module);

            for member in &module.module.members {
                match member {
                    ModuleMember::Function(function) => {
                        self.index_function(&module_context, function);
                    }
                    ModuleMember::Struct(move_struct) => {
                        self.index_struct(&module_context, move_struct);
                    }
                    ModuleMember::Enum(move_enum) => {
                        self.index_enum(&module_context, move_enum);
                    }
                    _ => {}
                }
            }
        }

        self.index_builtin_types();
    }

    fn collect_source_relationships(&mut self, modules: &[SourceModule]) {
        for module in modules {
            let mut module_context = module_context(module);
            module_context.aliases = module_aliases(&module.module, &module_context.address);

            for member in &module.module.members {
                match member {
                    ModuleMember::Function(function) => {
                        self.collect_function(&module_context, function);
                    }
                    ModuleMember::Struct(move_struct) => {
                        self.collect_struct(&module_context, move_struct);
                    }
                    ModuleMember::Enum(move_enum) => {
                        self.collect_enum(&module_context, move_enum);
                    }
                    _ => {}
                }
            }
        }
    }

    fn index_function(&mut self, module: &ModuleContext, function: &Function) {
        let visibility = function_visibility_name(&function.visibility);
        let is_entry = function.entry.is_some();
        let id = function_id(
            Some(&module.package_path),
            module.address.as_deref(),
            &module.module_name,
            &function.name.0.value.to_string(),
        );
        let node = MoveCallGraphNode {
            id: id.clone(),
            package_name: Some(module.package_name.clone()),
            package_path: Some(module.package_path.clone()),
            address: module.address.clone(),
            module_name: module.module_name.clone(),
            function_name: function.name.0.value.to_string(),
            qualified_name: qualified_member(
                module.address.as_deref(),
                &module.module_name,
                &function.name.0.value.to_string(),
            ),
            file_path: Some(module.file_path.clone()),
            visibility: visibility.clone(),
            is_entry,
            is_transaction_callable: is_entry || visibility == "public",
            attributes: ast_attributes(&function.attributes, &module.source),
            signature: Some(ast_function_signature(function, &module.source)),
            span: Some(source_span(
                &module.source,
                &module.file_path,
                function.loc.start() as usize,
                function.loc.end() as usize,
            )),
            is_external: false,
            source: "source".to_string(),
        };

        self.function_exact.insert(
            (
                module.address.clone(),
                module.module_name.clone(),
                node.function_name.clone(),
            ),
            id.clone(),
        );
        self.function_by_module_member
            .entry((module.module_name.clone(), node.function_name.clone()))
            .or_default()
            .insert(id.clone());
        self.call_nodes.insert(id, node);
    }

    fn index_struct(&mut self, module: &ModuleContext, move_struct: &StructDefinition) {
        let name = move_struct.name.0.value.to_string();
        let id = type_id(
            "struct",
            module.address.as_deref(),
            &module.module_name,
            &name,
        );
        let node = MoveTypeGraphNode {
            id: id.clone(),
            kind: "struct".to_string(),
            package_name: Some(module.package_name.clone()),
            package_path: Some(module.package_path.clone()),
            address: module.address.clone(),
            canonical_address: canonical_address(&self.address_mapping, module.address.as_deref()),
            module_name: Some(module.module_name.clone()),
            name: name.clone(),
            qualified_name: qualified_member(module.address.as_deref(), &module.module_name, &name),
            file_path: Some(module.file_path.clone()),
            abilities: move_struct
                .abilities
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            type_parameters: struct_type_parameters(move_struct),
            attributes: ast_attributes(&move_struct.attributes, &module.source),
            span: Some(source_span(
                &module.source,
                &module.file_path,
                move_struct.loc.start() as usize,
                move_struct.loc.end() as usize,
            )),
            source: "source".to_string(),
            is_external: false,
        };

        self.type_exact.insert(
            (
                module.address.clone(),
                module.module_name.clone(),
                name.clone(),
            ),
            id.clone(),
        );
        self.type_by_module_member
            .entry((module.module_name.clone(), name))
            .or_default()
            .insert(id.clone());
        self.type_nodes.insert(id, node);
    }

    fn index_enum(&mut self, module: &ModuleContext, move_enum: &EnumDefinition) {
        let name = move_enum.name.0.value.to_string();
        let id = type_id(
            "enum",
            module.address.as_deref(),
            &module.module_name,
            &name,
        );
        let node = MoveTypeGraphNode {
            id: id.clone(),
            kind: "enum".to_string(),
            package_name: Some(module.package_name.clone()),
            package_path: Some(module.package_path.clone()),
            address: module.address.clone(),
            canonical_address: canonical_address(&self.address_mapping, module.address.as_deref()),
            module_name: Some(module.module_name.clone()),
            name: name.clone(),
            qualified_name: qualified_member(module.address.as_deref(), &module.module_name, &name),
            file_path: Some(module.file_path.clone()),
            abilities: move_enum
                .abilities
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            type_parameters: enum_type_parameters(move_enum),
            attributes: ast_attributes(&move_enum.attributes, &module.source),
            span: Some(source_span(
                &module.source,
                &module.file_path,
                move_enum.loc.start() as usize,
                move_enum.loc.end() as usize,
            )),
            source: "source".to_string(),
            is_external: false,
        };

        self.type_exact.insert(
            (
                module.address.clone(),
                module.module_name.clone(),
                name.clone(),
            ),
            id.clone(),
        );
        self.type_by_module_member
            .entry((module.module_name.clone(), name))
            .or_default()
            .insert(id.clone());
        self.type_nodes.insert(id, node);
    }

    fn index_builtin_types(&mut self) {
        for builtin in BUILTIN_TYPES {
            let id = builtin_type_id(builtin);

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id,
                    kind: "builtin".to_string(),
                    package_name: None,
                    package_path: None,
                    address: None,
                    canonical_address: None,
                    module_name: None,
                    name: (*builtin).to_string(),
                    qualified_name: (*builtin).to_string(),
                    file_path: None,
                    abilities: builtin_abilities(builtin),
                    type_parameters: builtin_type_parameters(builtin),
                    attributes: builtin_attributes(builtin),
                    span: None,
                    source: "builtin".to_string(),
                    is_external: false,
                });
        }
    }

    fn collect_function(&mut self, module: &ModuleContext, function: &Function) {
        let function_name = function.name.0.value.to_string();
        let Some(function_id) = self.resolve_exact_function(
            module.address.as_deref(),
            &module.module_name,
            &function_name,
        ) else {
            return;
        };
        let mut type_parameters = BTreeMap::new();

        for (type_parameter, constraints) in &function.signature.type_parameters {
            let id = type_parameter_id(&function_id, &type_parameter.value.to_string());

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id: id.clone(),
                    kind: "typeParameter".to_string(),
                    package_name: Some(module.package_name.clone()),
                    package_path: Some(module.package_path.clone()),
                    address: module.address.clone(),
                    canonical_address: canonical_address(
                        &self.address_mapping,
                        module.address.as_deref(),
                    ),
                    module_name: Some(module.module_name.clone()),
                    name: type_parameter.value.to_string(),
                    qualified_name: format!("{function_id}::{}", type_parameter.value),
                    file_path: Some(module.file_path.clone()),
                    abilities: constraints
                        .iter()
                        .map(|ability| ability_name(&ability.value).to_string())
                        .collect(),
                    type_parameters: Vec::new(),
                    attributes: Vec::new(),
                    span: Some(source_span(
                        &module.source,
                        &module.file_path,
                        type_parameter.loc.start() as usize,
                        type_parameter.loc.end() as usize,
                    )),
                    source: "source".to_string(),
                    is_external: false,
                });
            self.record_type_edge(TypeEdgeInput {
                source: function_id.clone(),
                target: id.clone(),
                relationship: "typeParameter".to_string(),
                span: source_span(
                    &module.source,
                    &module.file_path,
                    type_parameter.loc.start() as usize,
                    type_parameter.loc.end() as usize,
                ),
                ..TypeEdgeInput::default()
            });
            type_parameters.insert(type_parameter.value.to_string(), id);
        }

        let type_context = TypeContext {
            owner_id: function_id.clone(),
            owner_name: Some(function_name.clone()),
            module: module.clone(),
            type_parameters,
        };

        for (_, parameter, parameter_type) in &function.signature.parameters {
            self.record_type_usage(
                &type_context,
                parameter_type,
                TypeUsageInput {
                    relationship: "parameter".to_string(),
                    parameter_name: Some(parameter.0.value.to_string()),
                    function_name: Some(function_name.clone()),
                    ..TypeUsageInput::default()
                },
            );
        }

        self.record_type_usage(
            &type_context,
            &function.signature.return_type,
            TypeUsageInput {
                relationship: "return".to_string(),
                function_name: Some(function_name.clone()),
                ..TypeUsageInput::default()
            },
        );

        if let FunctionBody_::Defined(sequence) = &function.body.value {
            let function_context = FunctionContext {
                module: module.clone(),
                function_id,
                function_name,
                type_parameters: type_context.type_parameters.clone(),
            };
            self.traverse_sequence(&function_context, &module.aliases, sequence);
        }
    }

    fn collect_struct(&mut self, module: &ModuleContext, move_struct: &StructDefinition) {
        let name = move_struct.name.0.value.to_string();
        let Some(struct_id) =
            self.resolve_exact_type(module.address.as_deref(), &module.module_name, &name)
        else {
            return;
        };
        let mut type_parameters = BTreeMap::new();

        for type_parameter in &move_struct.type_parameters {
            let id = type_parameter_id(&struct_id, &type_parameter.name.value.to_string());

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id: id.clone(),
                    kind: "typeParameter".to_string(),
                    package_name: Some(module.package_name.clone()),
                    package_path: Some(module.package_path.clone()),
                    address: module.address.clone(),
                    canonical_address: canonical_address(
                        &self.address_mapping,
                        module.address.as_deref(),
                    ),
                    module_name: Some(module.module_name.clone()),
                    name: type_parameter.name.value.to_string(),
                    qualified_name: format!("{struct_id}::{}", type_parameter.name.value),
                    file_path: Some(module.file_path.clone()),
                    abilities: type_parameter
                        .constraints
                        .iter()
                        .map(|ability| ability_name(&ability.value).to_string())
                        .collect(),
                    type_parameters: Vec::new(),
                    attributes: Vec::new(),
                    span: Some(source_span(
                        &module.source,
                        &module.file_path,
                        type_parameter.name.loc.start() as usize,
                        type_parameter.name.loc.end() as usize,
                    )),
                    source: "source".to_string(),
                    is_external: false,
                });
            self.record_type_edge(TypeEdgeInput {
                source: struct_id.clone(),
                target: id.clone(),
                relationship: if type_parameter.is_phantom {
                    "phantomTypeParameter"
                } else {
                    "typeParameter"
                }
                .to_string(),
                span: source_span(
                    &module.source,
                    &module.file_path,
                    type_parameter.name.loc.start() as usize,
                    type_parameter.name.loc.end() as usize,
                ),
                ..TypeEdgeInput::default()
            });
            type_parameters.insert(type_parameter.name.value.to_string(), id);
        }

        let type_context = TypeContext {
            owner_id: struct_id,
            owner_name: Some(name),
            module: module.clone(),
            type_parameters,
        };

        match &move_struct.fields {
            StructFields::Named(fields) => {
                for (_, field, field_type) in fields {
                    self.record_type_usage(
                        &type_context,
                        field_type,
                        TypeUsageInput {
                            relationship: "field".to_string(),
                            field_name: Some(field.0.value.to_string()),
                            ..TypeUsageInput::default()
                        },
                    );
                }
            }
            StructFields::Positional(fields) => {
                for (index, (_, field_type)) in fields.iter().enumerate() {
                    self.record_type_usage(
                        &type_context,
                        field_type,
                        TypeUsageInput {
                            relationship: "field".to_string(),
                            field_name: Some(index.to_string()),
                            ..TypeUsageInput::default()
                        },
                    );
                }
            }
            StructFields::Native(_) => {}
        }
    }

    fn collect_enum(&mut self, module: &ModuleContext, move_enum: &EnumDefinition) {
        let name = move_enum.name.0.value.to_string();
        let Some(enum_id) =
            self.resolve_exact_type(module.address.as_deref(), &module.module_name, &name)
        else {
            return;
        };
        let mut type_parameters = BTreeMap::new();

        for type_parameter in &move_enum.type_parameters {
            let id = type_parameter_id(&enum_id, &type_parameter.name.value.to_string());

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id: id.clone(),
                    kind: "typeParameter".to_string(),
                    package_name: Some(module.package_name.clone()),
                    package_path: Some(module.package_path.clone()),
                    address: module.address.clone(),
                    canonical_address: canonical_address(
                        &self.address_mapping,
                        module.address.as_deref(),
                    ),
                    module_name: Some(module.module_name.clone()),
                    name: type_parameter.name.value.to_string(),
                    qualified_name: format!("{enum_id}::{}", type_parameter.name.value),
                    file_path: Some(module.file_path.clone()),
                    abilities: type_parameter
                        .constraints
                        .iter()
                        .map(|ability| ability_name(&ability.value).to_string())
                        .collect(),
                    type_parameters: Vec::new(),
                    attributes: Vec::new(),
                    span: Some(source_span(
                        &module.source,
                        &module.file_path,
                        type_parameter.name.loc.start() as usize,
                        type_parameter.name.loc.end() as usize,
                    )),
                    source: "source".to_string(),
                    is_external: false,
                });
            self.record_type_edge(TypeEdgeInput {
                source: enum_id.clone(),
                target: id.clone(),
                relationship: if type_parameter.is_phantom {
                    "phantomTypeParameter"
                } else {
                    "typeParameter"
                }
                .to_string(),
                span: source_span(
                    &module.source,
                    &module.file_path,
                    type_parameter.name.loc.start() as usize,
                    type_parameter.name.loc.end() as usize,
                ),
                ..TypeEdgeInput::default()
            });
            type_parameters.insert(type_parameter.name.value.to_string(), id);
        }

        let type_context = TypeContext {
            owner_id: enum_id,
            owner_name: Some(name),
            module: module.clone(),
            type_parameters,
        };

        for variant in &move_enum.variants {
            let variant_name = variant.name.0.value.to_string();

            match &variant.fields {
                VariantFields::Named(fields) => {
                    for (_, field, field_type) in fields {
                        self.record_type_usage(
                            &type_context,
                            field_type,
                            TypeUsageInput {
                                relationship: "variantField".to_string(),
                                field_name: Some(field.0.value.to_string()),
                                variant_name: Some(variant_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                VariantFields::Positional(fields) => {
                    for (index, (_, field_type)) in fields.iter().enumerate() {
                        self.record_type_usage(
                            &type_context,
                            field_type,
                            TypeUsageInput {
                                relationship: "variantField".to_string(),
                                field_name: Some(index.to_string()),
                                variant_name: Some(variant_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                VariantFields::Empty => {}
            }
        }
    }

    fn traverse_sequence(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        sequence: &Sequence,
    ) {
        let mut scoped_aliases = aliases.clone();

        for use_decl in &sequence.0 {
            apply_use_decl(
                &mut scoped_aliases,
                use_decl,
                &function.module.address,
                &function.module.module_name,
            );
        }

        for item in &sequence.1 {
            match &item.value {
                SequenceItem_::Seq(exp) => self.traverse_exp(function, &scoped_aliases, exp),
                SequenceItem_::Declare(bindings, annotation) => {
                    self.traverse_bind_list(function, &scoped_aliases, &bindings.value);
                    if let Some(annotation) = annotation {
                        self.record_function_type_usage(
                            function,
                            annotation,
                            TypeUsageInput {
                                relationship: "annotation".to_string(),
                                function_name: Some(function.function_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                SequenceItem_::Bind(bindings, annotation, exp) => {
                    self.traverse_bind_list(function, &scoped_aliases, &bindings.value);
                    if let Some(annotation) = annotation {
                        self.record_function_type_usage(
                            function,
                            annotation,
                            TypeUsageInput {
                                relationship: "annotation".to_string(),
                                function_name: Some(function.function_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                    self.traverse_exp(function, &scoped_aliases, exp);
                }
            }
        }

        if let Some(exp) = sequence.3.as_ref() {
            self.traverse_exp(function, &scoped_aliases, exp);
        }
    }

    fn traverse_bind_list(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        bindings: &[Bind],
    ) {
        for bind in bindings {
            self.traverse_bind(function, aliases, bind);
        }
    }

    fn traverse_bind(&mut self, function: &FunctionContext, aliases: &AliasScope, bind: &Bind) {
        match &bind.value {
            Bind_::Var(_, _) => {}
            Bind_::Unpack(name, bindings) => {
                self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "destructuring",
                    source_span(
                        &function.module.source,
                        &function.module.file_path,
                        bind.loc.start() as usize,
                        bind.loc.end() as usize,
                    ),
                    None,
                );
                match bindings {
                    move_compiler::parser::ast::FieldBindings::Named(fields) => {
                        for field in fields {
                            if let move_compiler::parser::ast::Ellipsis::Binder((_, bind)) = field {
                                self.traverse_bind(function, aliases, bind);
                            }
                        }
                    }
                    move_compiler::parser::ast::FieldBindings::Positional(fields) => {
                        for field in fields {
                            if let move_compiler::parser::ast::Ellipsis::Binder(bind) = field {
                                self.traverse_bind(function, aliases, bind);
                            }
                        }
                    }
                }
            }
        }
    }

    fn traverse_exp(&mut self, function: &FunctionContext, aliases: &AliasScope, exp: &Exp) {
        match &exp.value {
            Exp_::Value(_) | Exp_::Name(_) | Exp_::Unit | Exp_::Spec(_) | Exp_::UnresolvedError => {
            }
            Exp_::Move(_, inner)
            | Exp_::Copy(_, inner)
            | Exp_::Parens(inner)
            | Exp_::Dereference(inner)
            | Exp_::UnaryExp(_, inner)
            | Exp_::Borrow(_, inner)
            | Exp_::DotUnresolved(_, inner) => self.traverse_exp(function, aliases, inner),
            Exp_::Call(name, arguments) => {
                self.record_call(function, aliases, name, exp, &arguments.value, "direct");
            }
            Exp_::Pack(name, fields) => {
                self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "construction",
                    source_span(
                        &function.module.source,
                        &function.module.file_path,
                        exp.loc.start() as usize,
                        exp.loc.end() as usize,
                    ),
                    Some(function.function_name.clone()),
                );

                for (_, field_exp) in fields {
                    self.traverse_exp(function, aliases, field_exp);
                }
            }
            Exp_::Vector(_, type_arguments, expressions) => {
                if let Some(type_arguments) = type_arguments {
                    for (index, type_argument) in type_arguments.iter().enumerate() {
                        self.record_function_type_usage(
                            function,
                            type_argument,
                            TypeUsageInput {
                                relationship: "vectorElement".to_string(),
                                function_name: Some(function.function_name.clone()),
                                type_argument_index: Some(index),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                for expression in &expressions.value {
                    self.traverse_exp(function, aliases, expression);
                }
            }
            Exp_::IfElse(condition, true_branch, false_branch) => {
                self.traverse_exp(function, aliases, condition);
                self.traverse_exp(function, aliases, true_branch);
                if let Some(false_branch) = false_branch {
                    self.traverse_exp(function, aliases, false_branch);
                }
            }
            Exp_::Match(subject, arms) => {
                self.traverse_exp(function, aliases, subject);
                for arm in &arms.value {
                    self.traverse_match_pattern(function, aliases, &arm.value.pattern);
                    if let Some(guard) = &arm.value.guard {
                        self.traverse_exp(function, aliases, guard);
                    }
                    self.traverse_exp(function, aliases, &arm.value.rhs);
                }
            }
            Exp_::While(condition, body) | Exp_::BinopExp(condition, _, body) => {
                self.traverse_exp(function, aliases, condition);
                self.traverse_exp(function, aliases, body);
            }
            Exp_::Loop(body) | Exp_::Labeled(_, body) => self.traverse_exp(function, aliases, body),
            Exp_::Block(sequence) => self.traverse_sequence(function, aliases, sequence),
            Exp_::Lambda(bindings, return_type, body) => {
                for (binding_list, annotation) in &bindings.value {
                    self.traverse_bind_list(function, aliases, &binding_list.value);
                    if let Some(annotation) = annotation {
                        self.record_function_type_usage(
                            function,
                            annotation,
                            TypeUsageInput {
                                relationship: "annotation".to_string(),
                                function_name: Some(function.function_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                if let Some(return_type) = return_type {
                    self.record_function_type_usage(
                        function,
                        return_type,
                        TypeUsageInput {
                            relationship: "return".to_string(),
                            function_name: Some(function.function_name.clone()),
                            ..TypeUsageInput::default()
                        },
                    );
                }
                self.traverse_exp(function, aliases, body);
            }
            Exp_::Quant(_, bindings, triggers, condition, body) => {
                for binding in &bindings.value {
                    self.traverse_bind(function, aliases, &binding.value.0);
                    self.traverse_exp(function, aliases, &binding.value.1);
                }
                for trigger_group in triggers {
                    for trigger in trigger_group {
                        self.traverse_exp(function, aliases, trigger);
                    }
                }
                if let Some(condition) = condition {
                    self.traverse_exp(function, aliases, condition);
                }
                self.traverse_exp(function, aliases, body);
            }
            Exp_::ExpList(expressions) => {
                for expression in expressions {
                    self.traverse_exp(function, aliases, expression);
                }
            }
            Exp_::Assign(left, right) => {
                self.traverse_exp(function, aliases, left);
                self.traverse_exp(function, aliases, right);
            }
            Exp_::Abort(value) | Exp_::Return(_, value) | Exp_::Break(_, value) => {
                if let Some(value) = value {
                    self.traverse_exp(function, aliases, value);
                }
            }
            Exp_::Continue(_) => {}
            Exp_::Dot(receiver, _, _) => self.traverse_exp(function, aliases, receiver),
            Exp_::DotCall(receiver, _, method, is_macro, type_arguments, arguments) => {
                self.traverse_exp(function, aliases, receiver);
                self.record_dot_call(
                    function,
                    aliases,
                    method,
                    is_macro.is_some(),
                    type_arguments.as_deref().unwrap_or(&[]),
                    &arguments.value,
                    exp,
                );
            }
            Exp_::Index(collection, indexes) => {
                self.traverse_exp(function, aliases, collection);
                for index in &indexes.value {
                    self.traverse_exp(function, aliases, index);
                }
            }
            Exp_::Cast(value, cast_type) => {
                self.traverse_exp(function, aliases, value);
                self.record_function_type_usage(
                    function,
                    cast_type,
                    TypeUsageInput {
                        relationship: "cast".to_string(),
                        function_name: Some(function.function_name.clone()),
                        ..TypeUsageInput::default()
                    },
                );
            }
            Exp_::Annotate(value, annotation) => {
                self.traverse_exp(function, aliases, value);
                self.record_function_type_usage(
                    function,
                    annotation,
                    TypeUsageInput {
                        relationship: "annotation".to_string(),
                        function_name: Some(function.function_name.clone()),
                        ..TypeUsageInput::default()
                    },
                );
            }
        }
    }

    fn traverse_match_pattern(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        pattern: &MatchPattern,
    ) {
        match &pattern.value {
            MatchPattern_::PositionalConstructor(name, fields) => {
                self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "destructuring",
                    source_span(
                        &function.module.source,
                        &function.module.file_path,
                        pattern.loc.start() as usize,
                        pattern.loc.end() as usize,
                    ),
                    Some(function.function_name.clone()),
                );

                for field in &fields.value {
                    if let move_compiler::parser::ast::Ellipsis::Binder(pattern) = field {
                        self.traverse_match_pattern(function, aliases, pattern);
                    }
                }
            }
            MatchPattern_::FieldConstructor(name, fields) => {
                self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "destructuring",
                    source_span(
                        &function.module.source,
                        &function.module.file_path,
                        pattern.loc.start() as usize,
                        pattern.loc.end() as usize,
                    ),
                    Some(function.function_name.clone()),
                );

                for field in &fields.value {
                    if let move_compiler::parser::ast::Ellipsis::Binder((_, pattern)) = field {
                        self.traverse_match_pattern(function, aliases, pattern);
                    }
                }
            }
            MatchPattern_::Name(_, _) | MatchPattern_::Literal(_) => {}
            MatchPattern_::Or(left, right) => {
                self.traverse_match_pattern(function, aliases, left);
                self.traverse_match_pattern(function, aliases, right);
            }
            MatchPattern_::At(_, inner) => self.traverse_match_pattern(function, aliases, inner),
        }
    }

    fn record_call(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
        exp: &Exp,
        arguments: &[Exp],
        default_kind: &str,
    ) {
        let parts = name_access_parts(name);
        let call_kind = if parts.iter().any(|part| part.is_macro) {
            "macro"
        } else {
            default_kind
        };
        let raw_target = name_access_to_string(name);
        let type_arguments = name_access_type_arguments(name, &function.module.source);
        let span = source_span(
            &function.module.source,
            &function.module.file_path,
            exp.loc.start() as usize,
            exp.loc.end() as usize,
        );

        for type_argument in name_access_types(name) {
            self.record_function_type_usage(
                function,
                type_argument,
                TypeUsageInput {
                    relationship: "callTypeArgument".to_string(),
                    function_name: Some(function.function_name.clone()),
                    ..TypeUsageInput::default()
                },
            );
        }

        match self.resolve_call_target(function, aliases, name) {
            ResolvedCall::Local(target_id) => {
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "high".to_string(),
                    raw_target,
                    type_arguments,
                    span,
                    is_external: false,
                    is_resolved: true,
                });
            }
            ResolvedCall::External(target) => {
                let target_id = self.ensure_external_call_node(&target, &raw_target, "source");
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "medium".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments,
                    span: span.clone(),
                    is_external: true,
                    is_resolved: false,
                });
                self.record_unresolved_call(
                    function,
                    raw_target,
                    call_kind,
                    span,
                    "target function is outside parsed source",
                );
            }
            ResolvedCall::Unresolved(reason) => {
                let target_id = self.ensure_unresolved_call_node(&raw_target);
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "low".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments,
                    span: span.clone(),
                    is_external: false,
                    is_resolved: false,
                });
                self.record_unresolved_call(function, raw_target, call_kind, span, reason);
            }
        }

        for argument in arguments {
            self.traverse_exp(function, aliases, argument);
        }
    }

    fn record_dot_call(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        method: &Name,
        is_macro: bool,
        type_arguments: &[Type],
        arguments: &[Exp],
        exp: &Exp,
    ) {
        let method_name = method.value.to_string();
        let raw_target = format!(".{method_name}");
        let call_kind = if is_macro { "methodMacro" } else { "method" };
        let span = source_span(
            &function.module.source,
            &function.module.file_path,
            exp.loc.start() as usize,
            exp.loc.end() as usize,
        );
        let type_argument_sources = type_arguments
            .iter()
            .map(|type_argument| ast_type_source(type_argument, &function.module.source))
            .collect::<Vec<_>>();

        for type_argument in type_arguments {
            self.record_function_type_usage(
                function,
                type_argument,
                TypeUsageInput {
                    relationship: "callTypeArgument".to_string(),
                    function_name: Some(function.function_name.clone()),
                    ..TypeUsageInput::default()
                },
            );
        }

        if let Some(target) = aliases.method_aliases.get(&method_name) {
            if let Some(target_id) = self.resolve_exact_function(
                target.address.as_deref(),
                &target.module,
                &target.member,
            ) {
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "high".to_string(),
                    raw_target,
                    type_arguments: type_argument_sources,
                    span,
                    is_external: false,
                    is_resolved: true,
                });
            } else {
                let target_id = self.ensure_external_call_node(target, &raw_target, "source");
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "medium".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments: type_argument_sources,
                    span: span.clone(),
                    is_external: true,
                    is_resolved: false,
                });
                self.record_unresolved_call(
                    function,
                    raw_target,
                    call_kind,
                    span,
                    "use fun method target is outside parsed source",
                );
            }
        } else {
            let target_id = self.ensure_unresolved_call_node(&raw_target);
            self.record_call_edge(CallEdgeInput {
                source: function.function_id.clone(),
                target: target_id,
                call_kind: call_kind.to_string(),
                confidence: "low".to_string(),
                raw_target: raw_target.clone(),
                type_arguments: type_argument_sources,
                span: span.clone(),
                is_external: false,
                is_resolved: false,
            });
            self.record_unresolved_call(
                function,
                raw_target,
                call_kind,
                span,
                "method receiver type is not inferred by the parser graph pass",
            );
        }

        for argument in arguments {
            self.traverse_exp(function, aliases, argument);
        }
    }

    fn record_construct_or_destructure(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
        relationship: &str,
        span: MoveSourceSpan,
        function_name: Option<String>,
    ) {
        let type_context = TypeContext {
            owner_id: function.function_id.clone(),
            owner_name: Some(function.function_name.clone()),
            module: function.module.clone(),
            type_parameters: function.type_parameters.clone(),
        };

        if let Some(type_use) =
            self.resolve_type_apply(&type_context, aliases, name, span.clone(), relationship)
        {
            self.record_type_edge(TypeEdgeInput {
                source: function.function_id.clone(),
                target: type_use.id,
                relationship: relationship.to_string(),
                function_name,
                span,
                confidence: "syntactic".to_string(),
                ..TypeEdgeInput::default()
            });
        }
    }

    fn record_function_type_usage(
        &mut self,
        function: &FunctionContext,
        type_: &Type,
        input: TypeUsageInput,
    ) {
        let type_context = TypeContext {
            owner_id: function.function_id.clone(),
            owner_name: Some(function.function_name.clone()),
            module: function.module.clone(),
            type_parameters: function.type_parameters.clone(),
        };

        self.record_type_usage(&type_context, type_, input);
    }

    fn record_type_usage(
        &mut self,
        context: &TypeContext,
        type_: &Type,
        input: TypeUsageInput,
    ) -> Vec<TypeUse> {
        match &type_.value {
            Type_::Apply(name) => {
                let span = source_span(
                    &context.module.source,
                    &context.module.file_path,
                    type_.loc.start() as usize,
                    type_.loc.end() as usize,
                );
                let type_expression = ast_type_source(type_, &context.module.source);
                let Some(type_use) = self.resolve_type_apply(
                    context,
                    &context.module.aliases,
                    name,
                    span.clone(),
                    &input.relationship,
                ) else {
                    return Vec::new();
                };

                self.record_type_edge(TypeEdgeInput {
                    source: context.owner_id.clone(),
                    target: type_use.id.clone(),
                    relationship: input.relationship.clone(),
                    field_name: input.field_name.clone(),
                    variant_name: input.variant_name.clone(),
                    function_name: input
                        .function_name
                        .clone()
                        .or_else(|| context.owner_name.clone()),
                    parameter_name: input.parameter_name.clone(),
                    type_argument_index: input.type_argument_index,
                    is_mutable: input.is_mutable,
                    is_reference: input.is_reference,
                    type_expression: Some(type_expression.clone()),
                    declaring_type_id: input
                        .declaring_type_id
                        .clone()
                        .or_else(|| Some(context.owner_id.clone())),
                    declaring_field_name: input
                        .declaring_field_name
                        .clone()
                        .or_else(|| input.field_name.clone()),
                    span: span.clone(),
                    confidence: "syntactic".to_string(),
                    evidence: type_usage_evidence(context, &type_use.id, &type_expression, &input),
                    ..TypeEdgeInput::default()
                });

                for (index, type_argument) in name_access_types(name).into_iter().enumerate() {
                    for argument in self.record_type_usage(
                        context,
                        type_argument,
                        TypeUsageInput {
                            relationship: "genericArgument".to_string(),
                            type_argument_index: Some(index),
                            field_name: input.field_name.clone(),
                            variant_name: input.variant_name.clone(),
                            function_name: input.function_name.clone(),
                            parameter_name: input.parameter_name.clone(),
                            declaring_type_id: input
                                .declaring_type_id
                                .clone()
                                .or_else(|| Some(context.owner_id.clone())),
                            declaring_field_name: input
                                .declaring_field_name
                                .clone()
                                .or_else(|| input.field_name.clone()),
                            ..TypeUsageInput::default()
                        },
                    ) {
                        let argument_expression =
                            ast_type_source(type_argument, &context.module.source);
                        let type_argument_name =
                            self.generic_argument_name_for(&type_use.id, index);
                        self.record_type_edge(TypeEdgeInput {
                            source: type_use.id.clone(),
                            target: argument.id,
                            relationship: "genericArgument".to_string(),
                            type_argument_index: Some(index),
                            type_expression: Some(argument_expression.clone()),
                            declaring_type_id: input
                                .declaring_type_id
                                .clone()
                                .or_else(|| Some(context.owner_id.clone())),
                            declaring_field_name: input
                                .declaring_field_name
                                .clone()
                                .or_else(|| input.field_name.clone()),
                            type_argument_name: type_argument_name.clone(),
                            span: source_span(
                                &context.module.source,
                                &context.module.file_path,
                                type_argument.loc.start() as usize,
                                type_argument.loc.end() as usize,
                            ),
                            confidence: "syntactic".to_string(),
                            evidence: generic_argument_evidence(
                                &type_use.id,
                                type_argument_name.as_deref(),
                                index,
                                &argument_expression,
                                &input,
                            ),
                            ..TypeEdgeInput::default()
                        });
                    }
                }

                vec![type_use]
            }
            Type_::Ref(is_mutable, inner) => self.record_type_usage(
                context,
                inner,
                TypeUsageInput {
                    is_mutable: *is_mutable || input.is_mutable,
                    is_reference: true,
                    ..input
                },
            ),
            Type_::Fun(parameters, return_type) => {
                let mut uses = Vec::new();

                for parameter in parameters {
                    uses.extend(self.record_type_usage(
                        context,
                        parameter,
                        TypeUsageInput {
                            relationship: input.relationship.clone(),
                            ..input.clone()
                        },
                    ));
                }
                uses.extend(self.record_type_usage(context, return_type, input));
                uses
            }
            Type_::Multiple(types) => types
                .iter()
                .flat_map(|type_| self.record_type_usage(context, type_, input.clone()))
                .collect(),
            Type_::Unit | Type_::UnresolvedError => Vec::new(),
        }
    }

    fn resolve_call_target(
        &self,
        function: &FunctionContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
    ) -> ResolvedCall {
        let parts = name_access_parts(name);

        match parts.as_slice() {
            [single] => {
                if let Some(member) = aliases.member_aliases.get(&single.name) {
                    return self.resolve_member_call(member);
                }

                if let Some(target_id) = self.resolve_exact_function(
                    function.module.address.as_deref(),
                    &function.module.module_name,
                    &single.name,
                ) {
                    ResolvedCall::Local(target_id)
                } else {
                    ResolvedCall::Unresolved(
                        "single-name call did not match a local function or imported member",
                    )
                }
            }
            [module, member] => {
                if let Some(module_ref) = aliases.module_aliases.get(&module.name) {
                    let target = MemberRef {
                        address: module_ref.address.clone(),
                        module: module_ref.module.clone(),
                        member: member.name.clone(),
                    };
                    return self.resolve_member_call(&target);
                }

                let target = MemberRef {
                    address: function.module.address.clone(),
                    module: module.name.clone(),
                    member: member.name.clone(),
                };
                self.resolve_member_call(&target)
            }
            [address, module, member] => {
                let target = MemberRef {
                    address: Some(address.name.clone()),
                    module: module.name.clone(),
                    member: member.name.clone(),
                };
                self.resolve_member_call(&target)
            }
            _ => ResolvedCall::Unresolved("call target path shape is not a Move function path"),
        }
    }

    fn resolve_member_call(&self, member: &MemberRef) -> ResolvedCall {
        if let Some(target_id) =
            self.resolve_exact_function(member.address.as_deref(), &member.module, &member.member)
        {
            ResolvedCall::Local(target_id)
        } else {
            ResolvedCall::External(member.clone())
        }
    }

    fn resolve_type_apply(
        &mut self,
        context: &TypeContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
        span: MoveSourceSpan,
        relationship: &str,
    ) -> Option<TypeUse> {
        let raw_type = name_access_to_string(name);
        let parts = name_access_parts(name);
        let type_ref = match parts.as_slice() {
            [single] if BUILTIN_TYPES.contains(&single.name.as_str()) => {
                return Some(TypeUse {
                    id: builtin_type_id(&single.name),
                });
            }
            [single] => {
                if let Some(type_parameter) = context.type_parameters.get(&single.name) {
                    return Some(TypeUse {
                        id: type_parameter.clone(),
                    });
                }

                if let Some(member) = aliases.member_aliases.get(&single.name) {
                    member.clone()
                } else if let Some(local_type) = self.resolve_exact_type(
                    context.module.address.as_deref(),
                    &context.module.module_name,
                    &single.name,
                ) {
                    return Some(TypeUse { id: local_type });
                } else {
                    self.record_unresolved_type(
                        context,
                        raw_type,
                        relationship,
                        span,
                        "single-name type did not match a builtin, type parameter, local type, or imported member",
                    );
                    return None;
                }
            }
            [module, member] => {
                if let Some(module_ref) = aliases.module_aliases.get(&module.name) {
                    MemberRef {
                        address: module_ref.address.clone(),
                        module: module_ref.module.clone(),
                        member: member.name.clone(),
                    }
                } else {
                    MemberRef {
                        address: context.module.address.clone(),
                        module: module.name.clone(),
                        member: member.name.clone(),
                    }
                }
            }
            [address, module, member] => MemberRef {
                address: Some(address.name.clone()),
                module: module.name.clone(),
                member: member.name.clone(),
            },
            _ => {
                self.record_unresolved_type(
                    context,
                    raw_type,
                    relationship,
                    span,
                    "type path shape is not supported by the graph resolver",
                );
                return None;
            }
        };

        if let Some(type_id) = self.resolve_exact_type(
            type_ref.address.as_deref(),
            &type_ref.module,
            &type_ref.member,
        ) {
            Some(TypeUse { id: type_id })
        } else {
            Some(TypeUse {
                id: self.ensure_external_type_node(
                    "datatype",
                    type_ref.address.as_deref(),
                    &type_ref.module,
                    &type_ref.member,
                    "source",
                ),
            })
        }
    }

    fn resolve_exact_function(
        &self,
        address: Option<&str>,
        module: &str,
        function: &str,
    ) -> Option<String> {
        let key = (
            address.map(str::to_string),
            module.to_string(),
            function.to_string(),
        );

        self.function_exact.get(&key).cloned().or_else(|| {
            address.is_none().then(|| {
                self.function_by_module_member
                    .get(&(module.to_string(), function.to_string()))
                    .and_then(|matches| {
                        if matches.len() == 1 {
                            matches.iter().next().cloned()
                        } else {
                            None
                        }
                    })
            })?
        })
    }

    fn resolve_exact_type(
        &self,
        address: Option<&str>,
        module: &str,
        name: &str,
    ) -> Option<String> {
        let key = (
            address.map(str::to_string),
            module.to_string(),
            name.to_string(),
        );

        self.type_exact.get(&key).cloned().or_else(|| {
            address.is_none().then(|| {
                self.type_by_module_member
                    .get(&(module.to_string(), name.to_string()))
                    .and_then(|matches| {
                        if matches.len() == 1 {
                            matches.iter().next().cloned()
                        } else {
                            None
                        }
                    })
            })?
        })
    }

    fn ensure_external_call_node(
        &mut self,
        target: &MemberRef,
        raw_target: &str,
        source: &str,
    ) -> String {
        let id = external_function_id(target.address.as_deref(), &target.module, &target.member);

        self.call_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveCallGraphNode {
                id: id.clone(),
                package_name: None,
                package_path: None,
                address: target.address.clone(),
                module_name: target.module.clone(),
                function_name: target.member.clone(),
                qualified_name: raw_target.to_string(),
                file_path: None,
                visibility: "unknown".to_string(),
                is_entry: false,
                is_transaction_callable: false,
                attributes: Vec::new(),
                signature: None,
                span: None,
                is_external: true,
                source: source.to_string(),
            });

        id
    }

    fn ensure_unresolved_call_node(&mut self, raw_target: &str) -> String {
        let id = unresolved_call_id(raw_target);
        let function_name = raw_target
            .trim_start_matches('.')
            .rsplit("::")
            .next()
            .unwrap_or(raw_target)
            .to_string();
        let module_name = raw_target
            .trim_start_matches('.')
            .rsplit_once("::")
            .and_then(|(prefix, _)| prefix.rsplit("::").next())
            .unwrap_or("unresolved")
            .to_string();

        self.call_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveCallGraphNode {
                id: id.clone(),
                package_name: None,
                package_path: None,
                address: None,
                module_name,
                function_name,
                qualified_name: raw_target.to_string(),
                file_path: None,
                visibility: "unknown".to_string(),
                is_entry: false,
                is_transaction_callable: false,
                attributes: Vec::new(),
                signature: None,
                span: None,
                is_external: false,
                source: "unresolved".to_string(),
            });

        id
    }

    fn ensure_external_type_node(
        &mut self,
        kind: &str,
        address: Option<&str>,
        module: &str,
        name: &str,
        source: &str,
    ) -> String {
        let id = external_type_id(kind, address, module, name);
        let canonical_address = canonical_address(&self.address_mapping, address);
        let abilities =
            well_known_external_type_abilities(address, canonical_address.as_deref(), module, name);

        self.type_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveTypeGraphNode {
                id: id.clone(),
                kind: kind.to_string(),
                package_name: None,
                package_path: None,
                address: address.map(str::to_string),
                canonical_address,
                module_name: Some(module.to_string()),
                name: name.to_string(),
                qualified_name: qualified_member(address, module, name),
                file_path: None,
                abilities,
                type_parameters: well_known_type_parameters(module, name),
                attributes: Vec::new(),
                span: None,
                source: source.to_string(),
                is_external: true,
            });

        id
    }

    fn generic_argument_name_for(&self, owner_type_id: &str, index: usize) -> Option<String> {
        self.type_nodes
            .get(owner_type_id)
            .and_then(|node| node.type_parameters.get(index))
            .map(|parameter| parameter.name.clone())
            .or_else(|| {
                if owner_type_id == builtin_type_id("vector") && index == 0 {
                    Some("T".to_string())
                } else {
                    None
                }
            })
    }

    fn record_call_edge(&mut self, input: CallEdgeInput) {
        let key = CallEdgeKey {
            source: input.source.clone(),
            target: input.target.clone(),
            call_kind: input.call_kind.clone(),
            raw_target: input.raw_target.clone(),
            type_arguments: input.type_arguments.clone(),
        };
        let edge = self
            .call_edges
            .entry(key)
            .or_insert_with(|| MoveCallGraphEdge {
                source: input.source,
                target: input.target,
                call_kind: input.call_kind,
                confidence: input.confidence,
                call_count: 0,
                raw_target: input.raw_target,
                type_arguments: input.type_arguments,
                source_spans: Vec::new(),
                is_external: input.is_external,
                is_resolved: input.is_resolved,
            });

        edge.call_count += 1;
        edge.source_spans.push(input.span);
    }

    fn record_type_edge(&mut self, input: TypeEdgeInput) {
        let key = TypeEdgeKey {
            source: input.source.clone(),
            target: input.target.clone(),
            relationship: input.relationship.clone(),
            field_name: input.field_name.clone(),
            variant_name: input.variant_name.clone(),
            function_name: input.function_name.clone(),
            parameter_name: input.parameter_name.clone(),
            type_argument_index: input.type_argument_index,
            is_mutable: input.is_mutable,
            is_reference: input.is_reference,
            declaring_type_id: input.declaring_type_id.clone(),
            declaring_field_name: input.declaring_field_name.clone(),
            type_argument_name: input.type_argument_name.clone(),
        };
        let evidence = if input.evidence.is_empty() {
            default_type_edge_evidence(&input)
        } else {
            input.evidence
        };
        let edge = self
            .type_edges
            .entry(key)
            .or_insert_with(|| MoveTypeGraphEdge {
                source: input.source,
                target: input.target,
                relationship: input.relationship,
                field_name: input.field_name,
                variant_name: input.variant_name,
                function_name: input.function_name,
                parameter_name: input.parameter_name,
                type_argument_index: input.type_argument_index,
                is_mutable: input.is_mutable,
                is_reference: input.is_reference,
                type_expression: input.type_expression,
                declaring_type_id: input.declaring_type_id,
                declaring_field_name: input.declaring_field_name,
                type_argument_name: input.type_argument_name,
                source_spans: Vec::new(),
                confidence: input.confidence,
                evidence,
            });

        edge.source_spans.push(input.span);
    }

    fn record_unresolved_call(
        &mut self,
        function: &FunctionContext,
        raw_target: String,
        call_kind: &str,
        span: MoveSourceSpan,
        reason: &str,
    ) {
        let key = UnresolvedCallKey {
            source: function.function_id.clone(),
            raw_target: raw_target.clone(),
            call_kind: call_kind.to_string(),
            file_path: function.module.file_path.clone(),
            reason: reason.to_string(),
        };
        let unresolved = self
            .unresolved_calls
            .entry(key)
            .or_insert_with(|| MoveUnresolvedCall {
                source: function.function_id.clone(),
                raw_target,
                call_kind: call_kind.to_string(),
                file_path: function.module.file_path.clone(),
                spans: Vec::new(),
                reason: reason.to_string(),
            });

        unresolved.spans.push(span);
    }

    fn record_unresolved_type(
        &mut self,
        context: &TypeContext,
        raw_type: String,
        relationship: &str,
        span: MoveSourceSpan,
        reason: &str,
    ) {
        let key = UnresolvedTypeKey {
            source: context.owner_id.clone(),
            raw_type: raw_type.clone(),
            context: relationship.to_string(),
            file_path: context.module.file_path.clone(),
            reason: reason.to_string(),
        };
        let unresolved = self
            .unresolved_types
            .entry(key)
            .or_insert_with(|| MoveUnresolvedType {
                source: context.owner_id.clone(),
                raw_type,
                context: relationship.to_string(),
                file_path: context.module.file_path.clone(),
                spans: Vec::new(),
                reason: reason.to_string(),
            });

        unresolved.spans.push(span);
    }

    fn enrich_from_summaries(&mut self, root: &Path, packages: &[MovePackageModel]) {
        let Some(summary_location) = resolve_summary_location(root, packages) else {
            return;
        };

        self.address_mapping =
            read_address_mapping(&summary_location.path.join("address_mapping.json"));
        let address_mapping = self.address_mapping.clone();

        for node in self.type_nodes.values_mut() {
            if node.canonical_address.is_none() {
                node.canonical_address =
                    canonical_address(&address_mapping, node.address.as_deref());
            }
        }

        let target_summary_ids = self.target_summary_ids(packages, &address_mapping);
        let summaries = read_summary_modules(&summary_location.path, &target_summary_ids);

        for summary in summaries {
            let module_address = summary.id.address.clone();
            let module_name = summary.id.name.clone();

            for (name, summary_struct) in &summary.structs {
                let source_id = self
                    .resolve_exact_type(Some(&module_address), &module_name, name)
                    .unwrap_or_else(|| {
                        self.ensure_external_type_node(
                            "struct",
                            Some(&module_address),
                            &module_name,
                            name,
                            "summary",
                        )
                    });

                for (field_name, field_type) in summary_fields(summary_struct) {
                    self.record_summary_type_value(
                        &source_id,
                        &field_type,
                        SummaryTypeUsageInput {
                            relationship: "field".to_string(),
                            field_name: Some(field_name.clone()),
                            declaring_type_id: Some(source_id.clone()),
                            declaring_field_name: Some(field_name),
                            span: summary_span(),
                            ..SummaryTypeUsageInput::default()
                        },
                    );
                }
            }

            for (name, summary_enum) in &summary.enums {
                let source_id = self
                    .resolve_exact_type(Some(&module_address), &module_name, name)
                    .unwrap_or_else(|| {
                        self.ensure_external_type_node(
                            "enum",
                            Some(&module_address),
                            &module_name,
                            name,
                            "summary",
                        )
                    });

                if let Some(variants) = summary_enum.get("variants").and_then(Value::as_object) {
                    for (variant_name, variant) in variants {
                        for (field_name, field_type) in summary_fields(variant) {
                            self.record_summary_type_value(
                                &source_id,
                                &field_type,
                                SummaryTypeUsageInput {
                                    relationship: "variantField".to_string(),
                                    field_name: Some(field_name.clone()),
                                    variant_name: Some(variant_name.clone()),
                                    declaring_type_id: Some(source_id.clone()),
                                    declaring_field_name: Some(field_name),
                                    span: summary_span(),
                                    ..SummaryTypeUsageInput::default()
                                },
                            );
                        }
                    }
                }
            }

            for (name, summary_function) in &summary.functions {
                let source_id = self
                    .resolve_exact_function(Some(&module_address), &module_name, name)
                    .unwrap_or_else(|| {
                        let member = MemberRef {
                            address: Some(module_address.clone()),
                            module: module_name.clone(),
                            member: name.clone(),
                        };
                        self.ensure_external_call_node(
                            &member,
                            &qualified_member(Some(&module_address), &module_name, name),
                            "summary",
                        )
                    });

                if let Some(parameters) =
                    summary_function.get("parameters").and_then(Value::as_array)
                {
                    for parameter in parameters {
                        let parameter_name = parameter
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string);

                        if let Some(type_value) = parameter.get("type_") {
                            self.record_summary_type_value(
                                &source_id,
                                type_value,
                                SummaryTypeUsageInput {
                                    relationship: "parameter".to_string(),
                                    parameter_name,
                                    function_name: Some(name.clone()),
                                    span: summary_span(),
                                    ..SummaryTypeUsageInput::default()
                                },
                            );
                        }
                    }
                }

                if let Some(returns) = summary_function.get("return_").and_then(Value::as_array) {
                    for return_type in returns {
                        self.record_summary_type_value(
                            &source_id,
                            return_type,
                            SummaryTypeUsageInput {
                                relationship: "return".to_string(),
                                function_name: Some(name.clone()),
                                span: summary_span(),
                                ..SummaryTypeUsageInput::default()
                            },
                        );
                    }
                }
            }
        }
    }

    fn target_summary_ids(
        &self,
        packages: &[MovePackageModel],
        address_mapping: &BTreeMap<String, String>,
    ) -> BTreeSet<String> {
        let mut ids = packages
            .iter()
            .map(|package| package.name.clone())
            .collect::<BTreeSet<_>>();

        for node in self.type_nodes.values().filter(|node| !node.is_external) {
            if let Some(address) = &node.address {
                ids.insert(address.clone());
            }
            if let Some(canonical_address) = &node.canonical_address {
                ids.insert(canonical_address.clone());
            }
        }

        for value in ids.clone() {
            if let Some(mapped) = address_mapping.get(&value) {
                ids.insert(mapped.clone());
            }
        }

        ids
    }

    fn record_summary_type_value(
        &mut self,
        owner_id: &str,
        value: &Value,
        input: SummaryTypeUsageInput,
    ) -> Vec<TypeUse> {
        let mut uses = Vec::new();

        match value {
            Value::String(name) => {
                let id = if BUILTIN_TYPES.contains(&name.as_str()) {
                    builtin_type_id(name)
                } else {
                    self.ensure_external_type_node("summaryType", None, "summary", name, "summary")
                };
                self.record_type_edge(TypeEdgeInput {
                    source: owner_id.to_string(),
                    target: id.clone(),
                    relationship: input.relationship,
                    field_name: input.field_name,
                    variant_name: input.variant_name,
                    function_name: input.function_name,
                    parameter_name: input.parameter_name,
                    type_argument_index: input.type_argument_index,
                    is_mutable: input.is_mutable,
                    is_reference: input.is_reference,
                    type_expression: input.type_expression.or_else(|| Some(name.clone())),
                    declaring_type_id: input.declaring_type_id,
                    declaring_field_name: input.declaring_field_name,
                    type_argument_name: input.type_argument_name,
                    span: input.span,
                    confidence: "heuristic".to_string(),
                    evidence: vec![format!(
                        "Package summary records `{}` as `{name}`.",
                        owner_id
                    )],
                });
                uses.push(TypeUse { id });
            }
            Value::Object(object) => {
                if let Some(datatype) = object.get("Datatype").and_then(Value::as_object) {
                    if let (Some(module), Some(name)) = (
                        datatype.get("module").and_then(Value::as_object),
                        datatype.get("name").and_then(Value::as_str),
                    ) {
                        let address = module.get("address").and_then(Value::as_str);
                        let module_name = module
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        let id = self
                            .resolve_exact_type(address, module_name, name)
                            .unwrap_or_else(|| {
                                self.ensure_external_type_node(
                                    "datatype",
                                    address,
                                    module_name,
                                    name,
                                    "summary",
                                )
                            });

                        self.record_type_edge(TypeEdgeInput {
                            source: owner_id.to_string(),
                            target: id.clone(),
                            relationship: input.relationship.clone(),
                            field_name: input.field_name.clone(),
                            variant_name: input.variant_name.clone(),
                            function_name: input.function_name.clone(),
                            parameter_name: input.parameter_name.clone(),
                            type_argument_index: input.type_argument_index,
                            is_mutable: input.is_mutable,
                            is_reference: input.is_reference,
                            type_expression: input
                                .type_expression
                                .clone()
                                .or_else(|| Some(qualified_member(address, module_name, name))),
                            declaring_type_id: input.declaring_type_id.clone(),
                            declaring_field_name: input.declaring_field_name.clone(),
                            type_argument_name: input.type_argument_name.clone(),
                            span: input.span.clone(),
                            confidence: "heuristic".to_string(),
                            evidence: vec![format!(
                                "Package summary records `{}` as `{}`.",
                                owner_id,
                                qualified_member(address, module_name, name)
                            )],
                        });
                        uses.push(TypeUse { id: id.clone() });

                        if let Some(type_arguments) =
                            datatype.get("type_arguments").and_then(Value::as_array)
                        {
                            for (index, type_argument) in type_arguments.iter().enumerate() {
                                if let Some(argument) = type_argument.get("argument") {
                                    for argument_use in self.record_summary_type_value(
                                        &id,
                                        argument,
                                        SummaryTypeUsageInput {
                                            relationship: "genericArgument".to_string(),
                                            type_argument_index: Some(index),
                                            type_argument_name: self
                                                .generic_argument_name_for(&id, index),
                                            declaring_type_id: input.declaring_type_id.clone(),
                                            declaring_field_name: input
                                                .declaring_field_name
                                                .clone(),
                                            span: input.span.clone(),
                                            ..SummaryTypeUsageInput::default()
                                        },
                                    ) {
                                        uses.push(argument_use);
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(inner) = object.get("vector") {
                    let vector_id = builtin_type_id("vector");
                    self.record_type_edge(TypeEdgeInput {
                        source: owner_id.to_string(),
                        target: vector_id.clone(),
                        relationship: input.relationship.clone(),
                        field_name: input.field_name.clone(),
                        variant_name: input.variant_name.clone(),
                        function_name: input.function_name.clone(),
                        parameter_name: input.parameter_name.clone(),
                        type_argument_index: input.type_argument_index,
                        is_mutable: input.is_mutable,
                        is_reference: input.is_reference,
                        type_expression: input
                            .type_expression
                            .clone()
                            .or_else(|| Some("vector".to_string())),
                        declaring_type_id: input.declaring_type_id.clone(),
                        declaring_field_name: input.declaring_field_name.clone(),
                        type_argument_name: input.type_argument_name.clone(),
                        span: input.span.clone(),
                        confidence: "heuristic".to_string(),
                        evidence: vec![format!(
                            "Package summary records `{owner_id}` as a vector storage type."
                        )],
                    });
                    uses.push(TypeUse {
                        id: vector_id.clone(),
                    });
                    uses.extend(self.record_summary_type_value(
                        &vector_id,
                        inner,
                        SummaryTypeUsageInput {
                            relationship: "genericArgument".to_string(),
                            type_argument_name: Some("T".to_string()),
                            declaring_type_id: input.declaring_type_id.clone(),
                            declaring_field_name: input.declaring_field_name.clone(),
                            span: input.span,
                            ..SummaryTypeUsageInput::default()
                        },
                    ));
                } else if let Some(reference) = object.get("Reference").and_then(Value::as_array) {
                    if let Some(inner) = reference.get(1) {
                        uses.extend(self.record_summary_type_value(
                            owner_id,
                            inner,
                            SummaryTypeUsageInput {
                                is_mutable:
                                    reference.first().and_then(Value::as_bool).unwrap_or(false),
                                is_reference: true,
                                ..input
                            },
                        ));
                    }
                } else {
                    for nested in object.values() {
                        uses.extend(self.record_summary_type_value(
                            owner_id,
                            nested,
                            input.clone(),
                        ));
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    uses.extend(self.record_summary_type_value(owner_id, value, input.clone()));
                }
            }
            _ => {}
        }

        uses
    }

    fn finish(self) -> (MoveCallGraph, MoveTypeGraph) {
        (
            finish_call_graph(
                self.call_nodes.into_values().collect(),
                self.call_edges.into_values().collect(),
                self.unresolved_calls.into_values().collect(),
            ),
            finish_type_graph(
                self.type_nodes.into_values().collect(),
                self.type_edges.into_values().collect(),
                self.unresolved_types.into_values().collect(),
            ),
        )
    }
}

#[derive(Clone)]
struct NamePart {
    name: String,
    is_macro: bool,
}

enum ResolvedCall {
    Local(String),
    External(MemberRef),
    Unresolved(&'static str),
}

#[derive(Default)]
struct CallEdgeInput {
    source: String,
    target: String,
    call_kind: String,
    confidence: String,
    raw_target: String,
    type_arguments: Vec<String>,
    span: MoveSourceSpan,
    is_external: bool,
    is_resolved: bool,
}

#[derive(Clone)]
struct TypeUsageInput {
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
}

impl Default for TypeUsageInput {
    fn default() -> Self {
        Self {
            relationship: "usage".to_string(),
            field_name: None,
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            declaring_type_id: None,
            declaring_field_name: None,
        }
    }
}

struct TypeEdgeInput {
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
    type_expression: Option<String>,
    declaring_type_id: Option<String>,
    declaring_field_name: Option<String>,
    type_argument_name: Option<String>,
    span: MoveSourceSpan,
    confidence: String,
    evidence: Vec<String>,
}

impl Default for TypeEdgeInput {
    fn default() -> Self {
        Self {
            source: String::new(),
            target: String::new(),
            relationship: String::new(),
            field_name: None,
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            type_expression: None,
            declaring_type_id: None,
            declaring_field_name: None,
            type_argument_name: None,
            span: MoveSourceSpan::default(),
            confidence: "syntactic".to_string(),
            evidence: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct SummaryTypeUsageInput {
    relationship: String,
    field_name: Option<String>,
    variant_name: Option<String>,
    function_name: Option<String>,
    parameter_name: Option<String>,
    type_argument_index: Option<usize>,
    is_mutable: bool,
    is_reference: bool,
    type_expression: Option<String>,
    declaring_type_id: Option<String>,
    declaring_field_name: Option<String>,
    type_argument_name: Option<String>,
    span: MoveSourceSpan,
}

impl Default for SummaryTypeUsageInput {
    fn default() -> Self {
        Self {
            relationship: "summaryUsage".to_string(),
            field_name: None,
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            type_expression: None,
            declaring_type_id: None,
            declaring_field_name: None,
            type_argument_name: None,
            span: summary_span(),
        }
    }
}

fn parse_source_modules(root: &Path, packages: &[MovePackageModel]) -> Vec<SourceModule> {
    let mut modules = Vec::new();

    for package in packages {
        let package_root = root.join(&package.path);
        let mut files = Vec::new();

        collect_move_files(&package_root.join("sources"), &mut files);
        files.sort();

        for path in files {
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            let package_config = PackageConfig {
                flavor: Flavor::Sui,
                ..PackageConfig::default()
            };
            let env = CompilationEnv::new(
                Flags::empty().set_silence_warnings(true),
                Vec::new(),
                Vec::new(),
                None,
                BTreeMap::new(),
                Some(package_config),
                None,
            );
            let Ok(definitions) = parse_file_string(&env, FileHash::new(&source), &source, None)
            else {
                continue;
            };
            let Some(file_path) = relative_path(root, &path) else {
                continue;
            };
            let source = Arc::<str>::from(source);

            for definition in definitions {
                collect_source_definition_modules(
                    package,
                    &file_path,
                    &source,
                    definition,
                    None,
                    &mut modules,
                );
            }
        }
    }

    modules.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.name.cmp(&right.name))
    });
    modules
}

fn collect_source_definition_modules(
    package: &MovePackageModel,
    file_path: &str,
    source: &Arc<str>,
    definition: Definition,
    inherited_address: Option<String>,
    modules: &mut Vec<SourceModule>,
) {
    match definition {
        Definition::Module(module) => {
            let address = module
                .address
                .as_ref()
                .map(leading_name_access_to_string)
                .or(inherited_address);
            let name = module.name.0.value.to_string();

            modules.push(SourceModule {
                package_name: package.name.clone(),
                package_path: package.path.clone(),
                address,
                name,
                file_path: file_path.to_string(),
                source: Arc::clone(source),
                module,
            });
        }
        Definition::Address(address) => {
            let inherited_address = leading_name_access_to_string(&address.addr);

            for module in address.modules {
                collect_source_definition_modules(
                    package,
                    file_path,
                    source,
                    Definition::Module(module),
                    Some(inherited_address.clone()),
                    modules,
                );
            }
        }
    }
}

fn collect_move_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_files(&path, files);
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("move"))
        {
            files.push(path);
        }
    }
}

fn module_context(module: &SourceModule) -> ModuleContext {
    ModuleContext {
        package_name: module.package_name.clone(),
        package_path: module.package_path.clone(),
        address: module.address.clone(),
        module_name: module.name.clone(),
        file_path: module.file_path.clone(),
        source: module.source.clone(),
        aliases: AliasScope::default(),
    }
}

fn module_aliases(module: &ModuleDefinition, current_address: &Option<String>) -> AliasScope {
    let mut aliases = AliasScope::default();

    for member in &module.members {
        if let ModuleMember::Use(use_decl) = member {
            apply_use_decl(
                &mut aliases,
                use_decl,
                current_address,
                &module.name.0.value.to_string(),
            );
        }
    }

    aliases
}

fn apply_use_decl(
    aliases: &mut AliasScope,
    use_decl: &UseDecl,
    current_address: &Option<String>,
    current_module: &str,
) {
    match &use_decl.use_ {
        Use::ModuleUse(module, module_use) => {
            apply_module_use(
                aliases,
                ModuleRef {
                    address: Some(leading_name_access_to_string(&module.value.address)),
                    module: module.value.module.0.value.to_string(),
                },
                module_use,
            );
        }
        Use::NestedModuleUses(address, modules) => {
            let address = leading_name_access_to_string(address);

            for (module, module_use) in modules {
                apply_module_use(
                    aliases,
                    ModuleRef {
                        address: Some(address.clone()),
                        module: module.0.value.to_string(),
                    },
                    module_use,
                );
            }
        }
        Use::Fun {
            function, method, ..
        } => {
            if let Some(target) =
                member_ref_from_name_access(function, current_address, current_module)
            {
                aliases
                    .method_aliases
                    .insert(method.value.to_string(), target);
            }
        }
        Use::Partial { .. } => {}
    }
}

fn apply_module_use(aliases: &mut AliasScope, module_ref: ModuleRef, module_use: &ModuleUse) {
    match module_use {
        ModuleUse::Module(alias) => {
            let alias = alias
                .map(|name| name.0.value.to_string())
                .unwrap_or_else(|| module_ref.module.clone());

            aliases.module_aliases.insert(alias, module_ref);
        }
        ModuleUse::Members(members) => {
            for (member, alias) in members {
                let alias = alias
                    .map(|name| name.value.to_string())
                    .unwrap_or_else(|| member.value.to_string());

                aliases.member_aliases.insert(
                    alias,
                    MemberRef {
                        address: module_ref.address.clone(),
                        module: module_ref.module.clone(),
                        member: member.value.to_string(),
                    },
                );
            }
        }
        ModuleUse::Partial { .. } => {}
    }
}

fn member_ref_from_name_access(
    name: &NameAccessChain,
    current_address: &Option<String>,
    current_module: &str,
) -> Option<MemberRef> {
    let parts = name_access_parts(name);

    match parts.as_slice() {
        [single] => Some(MemberRef {
            address: current_address.clone(),
            module: current_module.to_string(),
            member: single.name.clone(),
        }),
        [module, member] => Some(MemberRef {
            address: current_address.clone(),
            module: module.name.clone(),
            member: member.name.clone(),
        }),
        [address, module, member] => Some(MemberRef {
            address: Some(address.name.clone()),
            module: module.name.clone(),
            member: member.name.clone(),
        }),
        _ => None,
    }
}

fn name_access_parts(name: &NameAccessChain) -> Vec<NamePart> {
    match &name.value {
        NameAccessChain_::Single(entry) => vec![NamePart {
            name: entry.name.value.to_string(),
            is_macro: entry.is_macro.is_some(),
        }],
        NameAccessChain_::Path(path) => {
            let mut parts = vec![NamePart {
                name: leading_name_access_to_string(&path.root.name),
                is_macro: path.root.is_macro.is_some(),
            }];

            parts.extend(path.entries.iter().map(|entry| NamePart {
                name: entry.name.value.to_string(),
                is_macro: entry.is_macro.is_some(),
            }));
            parts
        }
    }
}

fn name_access_to_string(name: &NameAccessChain) -> String {
    name_access_parts(name)
        .into_iter()
        .map(|part| part.name)
        .collect::<Vec<_>>()
        .join("::")
}

fn name_access_type_arguments<'a>(name: &'a NameAccessChain, source: &str) -> Vec<String> {
    name_access_types(name)
        .into_iter()
        .map(|type_| ast_type_source(type_, source))
        .collect()
}

fn name_access_types(name: &NameAccessChain) -> Vec<&Type> {
    match &name.value {
        NameAccessChain_::Single(entry) => entry
            .tyargs
            .as_ref()
            .map(|types| types.value.iter().collect())
            .unwrap_or_default(),
        NameAccessChain_::Path(path) => {
            let mut types = path
                .root
                .tyargs
                .as_ref()
                .map(|types| types.value.iter().collect::<Vec<_>>())
                .unwrap_or_default();

            for entry in &path.entries {
                if let Some(entry_types) = &entry.tyargs {
                    types.extend(entry_types.value.iter());
                }
            }

            types
        }
    }
}

fn leading_name_access_to_string(access: &LeadingNameAccess) -> String {
    match &access.value {
        LeadingNameAccess_::AnonymousAddress(address) => format!("{address}"),
        LeadingNameAccess_::GlobalAddress(name) | LeadingNameAccess_::Name(name) => {
            name.value.to_string()
        }
    }
}

fn function_visibility_name(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public(_) => "public",
        Visibility::Friend(_) => "public(friend)",
        Visibility::Package(_) => "public(package)",
        Visibility::Internal => "private",
    }
    .to_string()
}

fn ability_name(ability: &Ability_) -> &'static str {
    match ability {
        Ability_::Copy => "copy",
        Ability_::Drop => "drop",
        Ability_::Store => "store",
        Ability_::Key => "key",
    }
}

fn builtin_abilities(name: &str) -> Vec<String> {
    match name {
        "address" | "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "u256" | "vector" => {
            copy_drop_store_abilities()
        }
        "signer" => vec!["drop".to_string()],
        _ => Vec::new(),
    }
}

fn struct_type_parameters(move_struct: &StructDefinition) -> Vec<MoveTypeParameter> {
    move_struct
        .type_parameters
        .iter()
        .map(|parameter| MoveTypeParameter {
            name: parameter.name.value.to_string(),
            abilities: parameter
                .constraints
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            is_phantom: parameter.is_phantom,
        })
        .collect()
}

fn enum_type_parameters(move_enum: &EnumDefinition) -> Vec<MoveTypeParameter> {
    move_enum
        .type_parameters
        .iter()
        .map(|parameter| MoveTypeParameter {
            name: parameter.name.value.to_string(),
            abilities: parameter
                .constraints
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            is_phantom: parameter.is_phantom,
        })
        .collect()
}

fn builtin_type_parameters(name: &str) -> Vec<MoveTypeParameter> {
    if name == "vector" {
        vec![MoveTypeParameter {
            name: "T".to_string(),
            abilities: Vec::new(),
            is_phantom: false,
        }]
    } else {
        Vec::new()
    }
}

fn builtin_attributes(name: &str) -> Vec<String> {
    if name == "vector" {
        vec!["conditional abilities".to_string()]
    } else {
        Vec::new()
    }
}

fn well_known_external_type_abilities(
    address: Option<&str>,
    canonical_address: Option<&str>,
    module: &str,
    name: &str,
) -> Vec<String> {
    if !is_framework_address(address) && !is_framework_address(canonical_address) {
        return Vec::new();
    }

    match (module, name) {
        ("object", "ID") => copy_drop_store_abilities(),
        ("object", "UID") => vec!["store".to_string()],
        ("table", "Table") => vec!["key".to_string(), "store".to_string()],
        _ => Vec::new(),
    }
}

fn well_known_type_parameters(module: &str, name: &str) -> Vec<MoveTypeParameter> {
    let parameter_names: &[&str] = match (module, name) {
        ("table", "Table") => &["K", "V"],
        ("coin", "Coin")
        | ("balance", "Balance")
        | ("bag", "Bag")
        | ("object_bag", "ObjectBag") => &["T"],
        ("vec_map", "VecMap") => &["K", "V"],
        ("vec_set", "VecSet") => &["K"],
        ("dynamic_field", "Field") | ("dynamic_object_field", "Wrapper") => &["Name", "Value"],
        _ => &[],
    };

    parameter_names
        .iter()
        .map(|name| MoveTypeParameter {
            name: (*name).to_string(),
            abilities: Vec::new(),
            is_phantom: false,
        })
        .collect()
}

fn copy_drop_store_abilities() -> Vec<String> {
    COPY_DROP_STORE_ABILITIES
        .iter()
        .map(|ability| (*ability).to_string())
        .collect()
}

fn default_type_edge_evidence(input: &TypeEdgeInput) -> Vec<String> {
    let mut evidence = Vec::new();

    if let Some(field_name) = &input.field_name {
        evidence.push(format!(
            "Field `{field_name}` declares this type relationship."
        ));
    } else if let Some(parameter_name) = &input.parameter_name {
        evidence.push(format!(
            "Function parameter `{parameter_name}` declares this type relationship."
        ));
    } else if let Some(function_name) = &input.function_name {
        evidence.push(format!(
            "Function `{function_name}` declares this type relationship."
        ));
    }

    if let Some(type_expression) = &input.type_expression {
        evidence.push(format!("Type expression: `{type_expression}`."));
    }

    if evidence.is_empty() {
        evidence.push(format!(
            "Syntactic `{}` relationship recorded by the Move parser.",
            input.relationship
        ));
    }

    evidence
}

fn type_usage_evidence(
    context: &TypeContext,
    target_id: &str,
    type_expression: &str,
    input: &TypeUsageInput,
) -> Vec<String> {
    let mut evidence = Vec::new();

    if let Some(field_name) = &input.field_name {
        evidence.push(format!(
            "Field `{field_name}` in `{}` is declared as `{type_expression}`.",
            context.owner_name.as_deref().unwrap_or(&context.owner_id)
        ));
    } else if let Some(parameter_name) = &input.parameter_name {
        evidence.push(format!(
            "Parameter `{parameter_name}` in `{}` is declared as `{type_expression}`.",
            context.owner_name.as_deref().unwrap_or(&context.owner_id)
        ));
    } else {
        evidence.push(format!(
            "`{}` references `{type_expression}`.",
            context.owner_name.as_deref().unwrap_or(&context.owner_id)
        ));
    }

    evidence.push(format!("Resolved target type id: `{target_id}`."));
    evidence
}

fn generic_argument_evidence(
    owner_type_id: &str,
    argument_name: Option<&str>,
    index: usize,
    argument_expression: &str,
    input: &TypeUsageInput,
) -> Vec<String> {
    let name = argument_name
        .map(|name| format!("`{name}`"))
        .unwrap_or_else(|| format!("index `{index}`"));
    let mut evidence = vec![format!(
        "Generic argument {name} of `{owner_type_id}` resolves to `{argument_expression}`."
    )];

    if let Some(field_name) = &input.field_name {
        evidence.push(format!("Argument appears inside field `{field_name}`."));
    }

    evidence
}

fn ast_attributes(attributes: &[Attributes], source: &str) -> Vec<String> {
    let mut names = Vec::new();

    for attribute_group in attributes {
        let snippet = source_for_range(
            source,
            attribute_group.loc.start() as usize,
            attribute_group.loc.end() as usize,
        )
        .unwrap_or_default();

        for attribute in &attribute_group.value.0 {
            let name = match &attribute.value {
                Attribute_::Mode { .. } => {
                    if snippet.contains("test_only") {
                        "test_only"
                    } else if snippet.contains("mode(test)") || snippet.contains("mode = test") {
                        "mode(test)"
                    } else {
                        attribute.value.attribute_name()
                    }
                }
                _ => attribute.value.attribute_name(),
            };

            names.push(name.to_string());
        }

        if snippet.contains("test_only") && !names.iter().any(|name| name == "test_only") {
            names.push("test_only".to_string());
        }
    }

    names.sort();
    names.dedup();
    names
}

fn ast_function_signature(function: &Function, source: &str) -> String {
    let start = function.loc.start() as usize;
    let end = match &function.body.value {
        FunctionBody_::Defined(_) => function.body.loc.start() as usize,
        FunctionBody_::Native => function.loc.end() as usize,
    };

    source_for_range(source, start, end)
        .unwrap_or_default()
        .trim()
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim()
        .to_string()
}

fn ast_type_source(type_: &Type, source: &str) -> String {
    source_for_range(source, type_.loc.start() as usize, type_.loc.end() as usize)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn source_for_range(source: &str, start: usize, end: usize) -> Option<String> {
    if start <= end && end <= source.len() {
        Some(source[start..end].to_string())
    } else {
        None
    }
}

fn source_span(source: &str, file_path: &str, start: usize, end: usize) -> MoveSourceSpan {
    MoveSourceSpan {
        file_path: file_path.to_string(),
        start_line: line_number_at(source, start),
        end_line: line_number_at(source, end),
        start_byte: start,
        end_byte: end,
    }
}

fn summary_span() -> MoveSourceSpan {
    MoveSourceSpan {
        file_path: "package_summaries".to_string(),
        start_line: 0,
        end_line: 0,
        start_byte: 0,
        end_byte: 0,
    }
}

fn line_number_at(source: &str, offset: usize) -> usize {
    source
        .get(..offset.min(source.len()))
        .unwrap_or_default()
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn qualified_member(address: Option<&str>, module: &str, member: &str) -> String {
    if let Some(address) = address {
        format!("{address}::{module}::{member}")
    } else {
        format!("{module}::{member}")
    }
}

fn canonical_address(
    address_mapping: &BTreeMap<String, String>,
    address: Option<&str>,
) -> Option<String> {
    address.and_then(|address| {
        address_mapping
            .get(address)
            .cloned()
            .or_else(|| address.starts_with("0x").then(|| address.to_string()))
    })
}

fn is_framework_address(address: Option<&str>) -> bool {
    let Some(address) = address.map(str::to_lowercase) else {
        return false;
    };

    matches!(address.as_str(), "std" | "sui" | "0x1" | "0x2")
        || address.ends_with("0000000000000000000000000000000000000000000000000000000000000001")
        || address.ends_with("0000000000000000000000000000000000000000000000000000000000000002")
}

struct SummaryLocation {
    path: PathBuf,
}

fn resolve_summary_location(root: &Path, packages: &[MovePackageModel]) -> Option<SummaryLocation> {
    let root_summary = root.join("package_summaries");

    if root_summary.is_dir() {
        return Some(SummaryLocation { path: root_summary });
    }

    packages
        .iter()
        .filter_map(|move_package| {
            let package_summary = root.join(&move_package.path).join("package_summaries");

            package_summary.is_dir().then_some(SummaryLocation {
                path: package_summary,
            })
        })
        .min_by(|left, right| {
            left.path
                .components()
                .count()
                .cmp(&right.path.components().count())
                .then_with(|| left.path.cmp(&right.path))
        })
}

fn read_address_mapping(path: &Path) -> BTreeMap<String, String> {
    let Ok(source) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };

    serde_json::from_str(&source).unwrap_or_default()
}

#[derive(Deserialize)]
struct SummaryModule {
    id: SummaryModuleId,
    #[serde(default)]
    functions: BTreeMap<String, Value>,
    #[serde(default)]
    structs: BTreeMap<String, Value>,
    #[serde(default)]
    enums: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct SummaryModuleId {
    address: String,
    name: String,
}

fn read_summary_modules(directory: &Path, target_ids: &BTreeSet<String>) -> Vec<SummaryModule> {
    let mut modules = Vec::new();
    collect_summary_modules(directory, directory, target_ids, &mut modules);
    modules.sort_by(|left, right| {
        left.id
            .address
            .cmp(&right.id.address)
            .then_with(|| left.id.name.cmp(&right.id.name))
    });
    modules
}

fn collect_summary_modules(
    root: &Path,
    directory: &Path,
    target_ids: &BTreeSet<String>,
    modules: &mut Vec<SummaryModule>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            if should_enter_summary_directory(root, &path, target_ids) {
                collect_summary_modules(root, &path, target_ids, modules);
            }
            continue;
        }

        if !file_type.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            || path
                .file_stem()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, "address_mapping" | "root_package_metadata"))
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(summary) = serde_json::from_str::<SummaryModule>(&source) else {
            continue;
        };

        modules.push(summary);
    }
}

fn should_enter_summary_directory(
    root: &Path,
    directory: &Path,
    target_ids: &BTreeSet<String>,
) -> bool {
    if target_ids.is_empty() {
        return true;
    }

    let Ok(relative) = directory.strip_prefix(root) else {
        return false;
    };
    let mut components = relative.components();
    let Some(first_component) = components.next() else {
        return true;
    };
    let Some(first_component) = first_component.as_os_str().to_str() else {
        return false;
    };

    target_ids.contains(first_component)
}

fn summary_fields(value: &Value) -> Vec<(String, Value)> {
    let Some(fields) = value
        .get("fields")
        .and_then(|fields| fields.get("fields"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    let mut result = fields
        .iter()
        .filter_map(|(field_name, field)| {
            field
                .get("type_")
                .cloned()
                .map(|field_type| (field_name.clone(), field_type))
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result
}

#[cfg(test)]
mod tests {
    use super::super::discover_move_project_model;
    use std::{fs, path::Path};
    use tempfile::tempdir;

    fn write_package(root: &Path, source: &str) {
        fs::write(
            root.join("Move.toml"),
            r#"
[package]
name = "demo"
"#,
        )
        .expect("manifest");
        fs::create_dir_all(root.join("sources")).expect("sources");
        fs::write(root.join("sources/main.move"), source).expect("source");
    }

    #[test]
    fn call_graph_resolves_local_qualified_alias_and_external_calls() {
        let temp = tempdir().expect("tempdir");
        write_package(
            temp.path(),
            r#"
module demo::helper {
    public fun ping() {}
}

module demo::main {
    use demo::helper;
    use demo::helper::{ping as imported_ping};

    fun local() {}

    public fun run() {
        local();
        helper::ping();
        imported_ping();
        sui::transfer::share_object(0);
    }
}
"#,
        );

        let project = discover_move_project_model(temp.path());
        let run_id = project
            .call_graph
            .nodes
            .iter()
            .find(|node| node.module_name == "main" && node.function_name == "run")
            .expect("run node")
            .id
            .clone();
        let local_id = project
            .call_graph
            .nodes
            .iter()
            .find(|node| node.module_name == "main" && node.function_name == "local")
            .expect("local node")
            .id
            .clone();
        let ping_id = project
            .call_graph
            .nodes
            .iter()
            .find(|node| node.module_name == "helper" && node.function_name == "ping")
            .expect("ping node")
            .id
            .clone();

        assert!(project
            .call_graph
            .edges
            .iter()
            .any(|edge| edge.source == run_id && edge.target == local_id && edge.is_resolved));
        assert!(
            project
                .call_graph
                .edges
                .iter()
                .filter(|edge| edge.source == run_id && edge.target == ping_id)
                .map(|edge| edge.call_count)
                .sum::<usize>()
                == 2
        );
        assert!(project
            .call_graph
            .unresolved_calls
            .iter()
            .any(|call| call.raw_target == "sui::transfer::share_object"));
    }

    #[test]
    fn call_graph_preserves_use_fun_methods_and_unknown_methods() {
        let temp = tempdir().expect("tempdir");
        write_package(
            temp.path(),
            r#"
module demo::main {
    public struct Box has drop { value: u64 }

    public fun get(box: &Box): u64 { box.value }

    use fun get as Box.value;

    public fun run(box: &Box) {
        box.value();
        box.unknown();
    }
}
"#,
        );

        let project = discover_move_project_model(temp.path());

        assert!(project
            .call_graph
            .edges
            .iter()
            .any(|edge| edge.call_kind == "method"
                && edge.raw_target == ".value"
                && edge.is_resolved));
        assert!(project
            .call_graph
            .unresolved_calls
            .iter()
            .any(|call| call.raw_target == ".unknown"));
        let node_ids = project
            .call_graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(project
            .call_graph
            .edges
            .iter()
            .all(|edge| node_ids.contains(edge.source.as_str())
                && node_ids.contains(edge.target.as_str())));
    }

    #[test]
    fn graph_resolution_respects_explicit_addresses() {
        let temp = tempdir().expect("tempdir");
        write_package(
            temp.path(),
            r#"
module demo::transfer {
    public fun share_object() {}
}

module demo::object {
    public struct UID has store {}
}

module demo::main {
    public struct Holder has store { id: sui::object::UID }

    public fun run() {
        sui::transfer::share_object();
    }
}
"#,
        );

        let project = discover_move_project_model(temp.path());
        let local_share_object_id = project
            .call_graph
            .nodes
            .iter()
            .find(|node| {
                node.module_name == "transfer"
                    && node.function_name == "share_object"
                    && !node.is_external
            })
            .expect("local transfer::share_object")
            .id
            .clone();

        assert!(!project
            .call_graph
            .edges
            .iter()
            .any(|edge| edge.target == local_share_object_id));
        assert!(project.call_graph.edges.iter().any(|edge| {
            edge.raw_target == "sui::transfer::share_object"
                && edge.is_external
                && !edge.is_resolved
        }));

        let local_uid_id = project
            .type_graph
            .nodes
            .iter()
            .find(|node| {
                node.module_name.as_deref() == Some("object")
                    && node.name == "UID"
                    && !node.is_external
            })
            .expect("local object::UID")
            .id
            .clone();

        assert!(!project.type_graph.edges.iter().any(|edge| {
            edge.relationship == "field"
                && edge.field_name.as_deref() == Some("id")
                && edge.target == local_uid_id
        }));
        assert!(project.type_graph.edges.iter().any(|edge| {
            edge.relationship == "field"
                && edge.field_name.as_deref() == Some("id")
                && project.type_graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.is_external
                        && node.qualified_name == "sui::object::UID"
                })
        }));
    }

    #[test]
    fn type_graph_extracts_fields_signatures_construction_and_annotations() {
        let temp = tempdir().expect("tempdir");
        write_package(
            temp.path(),
            r#"
module demo::main {
    public struct Coin<phantom T> has key, store { id: UID, amount: u64 }
    public struct Receipt has store { coin: Coin<u64> }

    public fun make(amount: u64): Receipt {
        let receipt: Receipt = Receipt { coin: Coin<u64> { id: object::new(), amount } };
        receipt
    }

    public fun unwrap(receipt: Receipt): Coin<u64> {
        let Receipt { coin } = receipt;
        coin
    }
}
"#,
        );

        let project = discover_move_project_model(temp.path());

        let builtin_abilities = |name: &str| {
            project
                .type_graph
                .nodes
                .iter()
                .find(|node| node.kind == "builtin" && node.name == name)
                .unwrap_or_else(|| panic!("builtin node {name}"))
                .abilities
                .clone()
        };

        assert_eq!(builtin_abilities("u64"), ["copy", "drop", "store"]);
        assert_eq!(builtin_abilities("bool"), ["copy", "drop", "store"]);
        assert_eq!(builtin_abilities("address"), ["copy", "drop", "store"]);
        assert_eq!(builtin_abilities("signer"), ["drop"]);
        assert_eq!(builtin_abilities("vector"), ["copy", "drop", "store"]);

        let coin_node = project
            .type_graph
            .nodes
            .iter()
            .find(|node| node.name == "Coin")
            .expect("Coin node");
        assert!(coin_node.span.is_some());
        assert_eq!(coin_node.type_parameters.len(), 1);
        assert_eq!(coin_node.type_parameters[0].name, "T");
        assert!(coin_node.type_parameters[0].is_phantom);

        assert!(
            project
                .type_graph
                .edges
                .iter()
                .any(|edge| edge.relationship == "field"
                    && edge.field_name.as_deref() == Some("coin"))
        );
        let receipt_coin_edge = project
            .type_graph
            .edges
            .iter()
            .find(|edge| edge.relationship == "field" && edge.field_name.as_deref() == Some("coin"))
            .expect("Receipt.coin field edge");
        assert_eq!(receipt_coin_edge.confidence, "syntactic");
        assert_eq!(
            receipt_coin_edge.type_expression.as_deref(),
            Some("Coin<u64>")
        );
        assert_eq!(
            receipt_coin_edge.declaring_field_name.as_deref(),
            Some("coin")
        );
        assert!(!receipt_coin_edge.source_spans.is_empty());
        assert!(!receipt_coin_edge.evidence.is_empty());

        assert!(project.type_graph.edges.iter().any(|edge| {
            edge.relationship == "genericArgument"
                && edge.type_argument_name.as_deref() == Some("T")
                && edge.type_expression.as_deref() == Some("u64")
                && edge.declaring_field_name.as_deref() == Some("coin")
        }));
        assert!(project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "return"));
        assert!(project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "construction"));
        assert!(project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "destructuring"));
        assert!(project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "annotation"));
        assert!(project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "genericArgument"));
    }

    #[test]
    fn summary_enrichment_adds_external_datatype_edges() {
        let temp = tempdir().expect("tempdir");
        write_package(
            temp.path(),
            r#"
module demo::main {
    public fun ping() {}
}
"#,
        );
        fs::create_dir_all(temp.path().join("package_summaries/demo")).expect("summary dir");
        fs::write(
            temp.path().join("package_summaries/address_mapping.json"),
            r#"{"demo":"0x1","external":"0x2","sui":"0x2"}"#,
        )
        .expect("mapping");
        fs::write(
            temp.path().join("package_summaries/demo/main.json"),
            r#"{
  "id": { "address": "demo", "name": "main" },
  "functions": {},
  "structs": {
    "Holder": {
      "fields": {
        "positional_fields": false,
        "fields": {
          "item": {
            "type_": {
              "Datatype": {
                "module": { "address": "external", "name": "asset" },
                "name": "Coin",
                "type_arguments": []
              }
            }
          },
          "id": {
            "type_": {
              "Datatype": {
                "module": { "address": "sui", "name": "object" },
                "name": "ID",
                "type_arguments": []
              }
            }
          },
          "uid": {
            "type_": {
              "Datatype": {
                "module": { "address": "sui", "name": "object" },
                "name": "UID",
                "type_arguments": []
              }
            }
          },
          "table": {
            "type_": {
              "Datatype": {
                "module": { "address": "sui", "name": "table" },
                "name": "Table",
                "type_arguments": []
              }
            }
          }
        }
      }
    }
  },
  "enums": {}
}"#,
        )
        .expect("summary");

        let project = discover_move_project_model(temp.path());

        assert!(project
            .type_graph
            .nodes
            .iter()
            .any(|node| node.is_external && node.qualified_name == "external::asset::Coin"));
        let summary_abilities = |qualified_name: &str| {
            project
                .type_graph
                .nodes
                .iter()
                .find(|node| node.qualified_name == qualified_name)
                .unwrap_or_else(|| panic!("summary node {qualified_name}"))
                .abilities
                .clone()
        };
        assert_eq!(
            summary_abilities("sui::object::ID"),
            ["copy", "drop", "store"]
        );
        assert_eq!(summary_abilities("sui::object::UID"), ["store"]);
        assert_eq!(summary_abilities("sui::table::Table"), ["key", "store"]);
        let table_node = project
            .type_graph
            .nodes
            .iter()
            .find(|node| node.qualified_name == "sui::table::Table")
            .expect("table node");
        assert_eq!(
            table_node
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            ["K", "V"]
        );
        assert!(
            project
                .type_graph
                .edges
                .iter()
                .any(|edge| edge.relationship == "field"
                    && edge.field_name.as_deref() == Some("item"))
        );
        assert!(project.type_graph.edges.iter().any(|edge| {
            edge.relationship == "field"
                && edge.field_name.as_deref() == Some("item")
                && edge.confidence == "heuristic"
                && !edge.evidence.is_empty()
        }));
    }
}
