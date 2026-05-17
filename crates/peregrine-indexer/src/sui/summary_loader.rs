use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    core::{
        estimate_tokens, is_neutral_tag, logical_id, stable_id, AddressMapping, Diagnostic,
        DiagnosticSeverity, Edge, EdgeType, FieldInfo, FunctionInfo, FunctionParameter,
        FunctionVisibility, MaterializedStatus, ModuleInfo, PackageInfo, PackageRole,
        PackageStatus, SemanticTag, SourceSpan, SummaryArtifact, TypeDef, TypeKind,
    },
    storage::sqlite::{SqliteIndexReader, SqliteIndexWriter},
    sui::model::{
        DependencyRecord, MaterializedSummaryContext, ModuleSummaryCard, ProgramIndex,
        SourceFileRecord, SummaryArtifacts, SummaryMaterializationRequest, SummaryPointerIndex,
    },
};

pub fn extract_summary_pointers(
    artifacts: SummaryArtifacts,
    debug_store_raw_summary_json: bool,
) -> crate::core::IndexerResult<SummaryPointerIndex> {
    let now = unix_timestamp();
    let package_hash = artifacts.package.manifest_hash.clone();
    let package_id = logical_id(
        "package",
        [&artifacts.package.package_name, &package_hash[..16]],
    );
    let mut program = ProgramIndex {
        package: PackageInfo {
            id: package_id.clone(),
            name: artifacts.package.package_name.clone(),
            root_path: artifacts.package.root.to_string_lossy().into_owned(),
            manifest_path: artifacts
                .package
                .manifest_path
                .to_string_lossy()
                .into_owned(),
            role: PackageRole::Root,
            compiler_version: None,
            package_hash,
            status: PackageStatus::Indexed,
            indexed_at: now,
            metadata_json: Some(json!({
                "summary_root": artifacts.summary_root.as_ref().map(|path| path.to_string_lossy().into_owned()),
                "root_metadata_path": artifacts.root_metadata_path.as_ref().map(|path| path.to_string_lossy().into_owned()),
            })),
        },
        files: vec![SourceFileRecord {
            id: logical_id("file", [&artifacts.package.package_name, "Move.toml"]),
            path: artifacts
                .package
                .manifest_path
                .to_string_lossy()
                .into_owned(),
            content_hash: Some(artifacts.package.manifest_hash.clone()),
            kind: "manifest".to_string(),
        }],
        ..ProgramIndex::empty()
    };

    if let Some(path) = &artifacts.address_mapping_path {
        program.files.push(SourceFileRecord {
            id: logical_id("file", [&package_id, "address_mapping.json"]),
            path: path.to_string_lossy().into_owned(),
            content_hash: crate::sui::package_loader::content_hash_or_empty(path).into(),
            kind: "address_mapping".to_string(),
        });
        program
            .address_mappings
            .extend(read_address_mapping(path, &package_id));
    }
    if let Some(path) = &artifacts.root_metadata_path {
        let root_metadata_hash = crate::sui::package_loader::content_hash_or_empty(path);
        program.files.push(SourceFileRecord {
            id: logical_id("file", [&package_id, "root_package_metadata.json"]),
            path: path.to_string_lossy().into_owned(),
            content_hash: Some(root_metadata_hash.clone()),
            kind: "root_package_metadata".to_string(),
        });
        if let Some(metadata) = read_compact_json(path) {
            let mut package_metadata = program
                .package
                .metadata_json
                .take()
                .unwrap_or_else(|| json!({}));
            if let Some(object) = package_metadata.as_object_mut() {
                object.insert(
                    "root_metadata_path".to_string(),
                    Value::String(path.to_string_lossy().into_owned()),
                );
                object.insert(
                    "root_metadata_hash".to_string(),
                    Value::String(root_metadata_hash),
                );
                object.insert("root_package_metadata".to_string(), metadata);
            }
            program.package.metadata_json = Some(package_metadata);
        }
    }

    let root_alias = artifacts.package.package_name.clone();
    let mut parsed = Vec::new();
    let mut direct_dependency_modules = BTreeSet::new();

    for path in &artifacts.summary_files {
        let hash = crate::sui::package_loader::content_hash_or_empty(path);
        match read_summary_module(path) {
            Ok(summary) => {
                if summary.id.address == root_alias {
                    for dep in &summary.immediate_dependencies {
                        if dep.address != root_alias {
                            direct_dependency_modules
                                .insert((dep.address.clone(), dep.name.clone()));
                        }
                    }
                }
                parsed.push((path.clone(), hash, Some(summary)));
            }
            Err(error) => {
                let (package_alias, module_name) =
                    derived_summary_identity(artifacts.summary_root.as_deref(), path);
                let artifact = summary_artifact(
                    &package_id,
                    package_alias,
                    module_name,
                    path,
                    hash,
                    PackageRole::UnknownDependency,
                    MaterializedStatus::PointerOnly,
                    None,
                    now,
                );
                program.summary_artifacts.push(artifact.clone());
                program.diagnostics.push(Diagnostic {
                    id: stable_id("diagnostic", [&artifact.id, "malformed-summary"]),
                    package_id: package_id.clone(),
                    severity: DiagnosticSeverity::Warning,
                    source: "sui.summary_loader".to_string(),
                    message: format!("Malformed package summary: {error}"),
                    source_span: SourceSpan::summary_artifact(artifact.id),
                    metadata_json: Some(json!({
                        "summary_path": path.to_string_lossy(),
                    })),
                });
            }
        }
    }

    for (path, hash, summary) in parsed {
        let Some(summary) = summary else {
            continue;
        };
        let role = classify_package_role(&summary.id.address, &root_alias);
        let status = if summary.id.address == root_alias {
            MaterializedStatus::RootCard
        } else if direct_dependency_modules
            .contains(&(summary.id.address.clone(), summary.id.name.clone()))
        {
            MaterializedStatus::DirectDependencyCard
        } else {
            MaterializedStatus::PointerOnly
        };
        let artifact_id =
            summary_artifact_id(&package_id, &summary.id.address, &summary.id.name, &hash);
        let span = SourceSpan::summary_artifact(artifact_id.clone());
        let card = match status {
            MaterializedStatus::RootCard | MaterializedStatus::DirectDependencyCard => {
                Some(module_card_json(
                    &summary,
                    status == MaterializedStatus::DirectDependencyCard,
                    None,
                ))
            }
            _ => None,
        };
        let mut artifact = SummaryArtifact {
            id: artifact_id.clone(),
            package_id: package_id.clone(),
            package_alias: summary.id.address.clone(),
            module_name: summary.id.name.clone(),
            summary_path: path.to_string_lossy().into_owned(),
            content_hash: hash,
            schema_version: summary.schema_version.clone(),
            role: role.clone(),
            materialized_status: status.clone(),
            last_seen_at: now,
            card_json: card,
        };

        if debug_store_raw_summary_json {
            artifact.card_json = artifact.card_json.map(|mut card| {
                if let Ok(source) = fs::read_to_string(&path) {
                    card["debug_raw_summary_json"] =
                        serde_json::from_str(&source).unwrap_or(Value::String(source));
                }
                card
            });
        }

        if summary.id.address == root_alias {
            materialize_root_summary(&mut program, &summary, &artifact, span.clone());
        }
        if summary.id.address == root_alias || status == MaterializedStatus::DirectDependencyCard {
            for dep in &summary.immediate_dependencies {
                if dep.address == summary.id.address && dep.name == summary.id.name {
                    continue;
                }
                program.dependencies.push(DependencyRecord {
                    id: stable_id(
                        "dependency",
                        [
                            &package_id,
                            &summary.id.address,
                            &summary.id.name,
                            &dep.address,
                            &dep.name,
                        ],
                    ),
                    package_id: package_id.clone(),
                    source_package_alias: summary.id.address.clone(),
                    source_module: summary.id.name.clone(),
                    target_package_alias: dep.address.clone(),
                    target_module: dep.name.clone(),
                    dependency_kind: "ImmediateModuleDependency".to_string(),
                    metadata_json: None,
                });
            }
        }

        program.summary_artifacts.push(artifact);
    }

    program.summary_artifacts.sort_by(|left, right| {
        left.package_alias
            .cmp(&right.package_alias)
            .then_with(|| left.module_name.cmp(&right.module_name))
    });
    program
        .diagnostics
        .sort_by(|left, right| left.id.cmp(&right.id));
    if !program.diagnostics.is_empty() {
        program.package.status = PackageStatus::PartialWithDiagnostics;
    }

    Ok(SummaryPointerIndex {
        program_index: program,
        summary_root: artifacts.summary_root,
    })
}

pub fn materialize_summary_context(
    request: SummaryMaterializationRequest,
) -> crate::core::IndexerResult<MaterializedSummaryContext> {
    let reader = SqliteIndexReader::open(&request.db_path)?;
    let pointer = reader
        .get_summary_artifact_pointer(&request.package_alias, &request.module_name)?
        .ok_or_else(|| {
            format!(
                "Summary artifact not found for {}::{}",
                request.package_alias, request.module_name
            )
        })?;
    let summary = read_summary_module(Path::new(&pointer.summary_path))?;
    let mut card_json = module_card_json(&summary, true, request.symbol_name.as_deref());
    let mut text = serde_json::to_string(&card_json)?;
    let mut estimated_tokens = estimate_tokens(&text);
    let mut trimmed = false;
    let mut trim_reasons = Vec::new();

    if estimated_tokens > request.budget.max_tokens_estimate {
        trimmed = true;
        trim_reasons.push("selected_public_symbols_trimmed".to_string());
        if let Some(symbols) = card_json
            .get_mut("selected_public_symbols")
            .and_then(Value::as_array_mut)
        {
            symbols.truncate(8);
        }
        text = serde_json::to_string(&card_json)?;
        estimated_tokens = estimate_tokens(&text);
    }
    if estimated_tokens > request.budget.max_tokens_estimate {
        trimmed = true;
        trim_reasons.push("types_trimmed".to_string());
        card_json["types"] = json!([]);
        text = serde_json::to_string(&card_json)?;
        estimated_tokens = estimate_tokens(&text);
    }

    let materialized_status = if request.symbol_name.is_some() {
        "ExpandedSymbol"
    } else {
        "ExpandedModule"
    };
    let writer = SqliteIndexWriter::open(&request.db_path)?;
    writer.update_summary_card(&pointer.artifact_id, materialized_status, &card_json)?;

    Ok(MaterializedSummaryContext {
        card: ModuleSummaryCard {
            artifact_id: pointer.artifact_id,
            package_alias: pointer.package_alias,
            module_name: pointer.module_name,
            summary_path: pointer.summary_path,
            content_hash: pointer.content_hash,
            role: pointer.role,
            materialized_status: materialized_status.to_string(),
            card: Some(card_json),
            estimated_tokens,
            budget_tokens: request.budget.max_tokens_estimate,
            trimmed,
            trim_reasons,
        },
    })
}

impl ProgramIndex {
    pub fn empty() -> Self {
        Self {
            package: PackageInfo {
                id: String::new(),
                name: String::new(),
                root_path: String::new(),
                manifest_path: String::new(),
                role: PackageRole::Root,
                compiler_version: None,
                package_hash: String::new(),
                status: PackageStatus::Indexed,
                indexed_at: 0,
                metadata_json: None,
            },
            files: Vec::new(),
            summary_artifacts: Vec::new(),
            address_mappings: Vec::new(),
            modules: Vec::new(),
            dependencies: Vec::new(),
            types: Vec::new(),
            fields: Vec::new(),
            functions: Vec::new(),
            locals: Vec::new(),
            basic_blocks: Vec::new(),
            operations: Vec::new(),
            edges: Vec::new(),
            semantic_tags: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SummaryModule {
    id: SummaryModuleId,
    #[serde(default)]
    immediate_dependencies: Vec<SummaryModuleId>,
    #[serde(default)]
    functions: BTreeMap<String, Value>,
    #[serde(default)]
    structs: BTreeMap<String, Value>,
    #[serde(default)]
    enums: BTreeMap<String, Value>,
    #[serde(default)]
    friends: Vec<SummaryModuleId>,
    #[serde(default)]
    docs: Option<String>,
    #[serde(default)]
    attributes: Value,
    #[serde(default)]
    schema_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct SummaryModuleId {
    address: String,
    name: String,
}

fn read_summary_module(path: &Path) -> crate::core::IndexerResult<SummaryModule> {
    let source = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&source)?)
}

fn materialize_root_summary(
    program: &mut ProgramIndex,
    summary: &SummaryModule,
    artifact: &SummaryArtifact,
    span: SourceSpan,
) {
    let package_id = program.package.id.clone();
    let module_id = logical_id(
        "module",
        [&package_id, &summary.id.address, &summary.id.name],
    );
    let module_full_name = format!("{}::{}", summary.id.address, summary.id.name);
    let immediate_dependencies = summary
        .immediate_dependencies
        .iter()
        .map(|dep| format!("{}::{}", dep.address, dep.name))
        .collect::<Vec<_>>();

    program.modules.push(ModuleInfo {
        id: module_id.clone(),
        package_id: package_id.clone(),
        summary_artifact_id: Some(artifact.id.clone()),
        file_id: None,
        address: summary.id.address.clone(),
        name: summary.id.name.clone(),
        full_name: module_full_name.clone(),
        immediate_dependencies,
        docs: summary.docs.clone(),
        attributes: value_array_strings(&summary.attributes),
        source_span: span.clone(),
    });
    program.edges.push(Edge {
        id: stable_id(
            "edge",
            [
                &package_id,
                &artifact.id,
                &module_id,
                "MATERIALIZED_FROM_SUMMARY",
            ],
        ),
        package_id: package_id.clone(),
        from_id: module_id.clone(),
        to_id: artifact.id.clone(),
        edge_type: EdgeType::MaterializedFromSummary,
        operation_id: None,
        source_span: span.clone(),
        metadata_json: None,
    });
    program.edges.push(Edge {
        id: stable_id(
            "edge",
            [
                &package_id,
                &program.package.id,
                &artifact.id,
                "HAS_SUMMARY_ARTIFACT",
            ],
        ),
        package_id: package_id.clone(),
        from_id: program.package.id.clone(),
        to_id: artifact.id.clone(),
        edge_type: EdgeType::HasSummaryArtifact,
        operation_id: None,
        source_span: span.clone(),
        metadata_json: None,
    });

    for dep in &summary.immediate_dependencies {
        program.edges.push(Edge {
            id: stable_id(
                "edge",
                [
                    &package_id,
                    &module_id,
                    &dep.address,
                    &dep.name,
                    "DEPENDS_ON_MODULE",
                ],
            ),
            package_id: package_id.clone(),
            from_id: module_id.clone(),
            to_id: format!("{}::{}", dep.address, dep.name),
            edge_type: EdgeType::DependsOnModule,
            operation_id: None,
            source_span: span.clone(),
            metadata_json: None,
        });
    }

    for friend in &summary.friends {
        program.edges.push(Edge {
            id: stable_id(
                "edge",
                [
                    &package_id,
                    &module_id,
                    &friend.address,
                    &friend.name,
                    "FRIENDS",
                ],
            ),
            package_id: package_id.clone(),
            from_id: module_id.clone(),
            to_id: format!("{}::{}", friend.address, friend.name),
            edge_type: EdgeType::Friends,
            operation_id: None,
            source_span: span.clone(),
            metadata_json: None,
        });
    }

    for (name, value) in &summary.structs {
        let type_def = type_from_summary(
            &package_id,
            &module_id,
            &module_full_name,
            name,
            value,
            TypeKind::Struct,
            span.clone(),
        );
        emit_type_tags(program, &type_def);
        program.edges.push(Edge {
            id: stable_id(
                "edge",
                [&package_id, &module_id, &type_def.id, "DEFINES_TYPE"],
            ),
            package_id: package_id.clone(),
            from_id: module_id.clone(),
            to_id: type_def.id.clone(),
            edge_type: EdgeType::DefinesType,
            operation_id: None,
            source_span: span.clone(),
            metadata_json: None,
        });
        for field in &type_def.fields {
            program.edges.push(Edge {
                id: stable_id("edge", [&package_id, &type_def.id, &field.id, "HAS_FIELD"]),
                package_id: package_id.clone(),
                from_id: type_def.id.clone(),
                to_id: field.id.clone(),
                edge_type: EdgeType::HasField,
                operation_id: None,
                source_span: field.source_span.clone(),
                metadata_json: Some(json!({ "type": field.type_name })),
            });
        }
        program.types.push(type_def);
    }

    for (name, value) in &summary.enums {
        let type_def = type_from_summary(
            &package_id,
            &module_id,
            &module_full_name,
            name,
            value,
            TypeKind::Enum,
            span.clone(),
        );
        emit_type_tags(program, &type_def);
        program.edges.push(Edge {
            id: stable_id(
                "edge",
                [&package_id, &module_id, &type_def.id, "DEFINES_TYPE"],
            ),
            package_id: package_id.clone(),
            from_id: module_id.clone(),
            to_id: type_def.id.clone(),
            edge_type: EdgeType::DefinesType,
            operation_id: None,
            source_span: span.clone(),
            metadata_json: None,
        });
        for field in &type_def.fields {
            program.edges.push(Edge {
                id: stable_id("edge", [&package_id, &type_def.id, &field.id, "HAS_FIELD"]),
                package_id: package_id.clone(),
                from_id: type_def.id.clone(),
                to_id: field.id.clone(),
                edge_type: EdgeType::HasField,
                operation_id: None,
                source_span: field.source_span.clone(),
                metadata_json: Some(json!({ "type": field.type_name })),
            });
        }
        program.types.push(type_def);
    }

    for (name, value) in &summary.functions {
        let function = function_from_summary(
            &package_id,
            &module_id,
            &module_full_name,
            name,
            value,
            span.clone(),
        );
        emit_function_tags(program, &function);
        program.edges.push(Edge {
            id: stable_id(
                "edge",
                [&package_id, &module_id, &function.id, "DEFINES_FUNCTION"],
            ),
            package_id: package_id.clone(),
            from_id: module_id.clone(),
            to_id: function.id.clone(),
            edge_type: EdgeType::DefinesFunction,
            operation_id: None,
            source_span: span.clone(),
            metadata_json: None,
        });
        for parameter in &function.parameters {
            program.edges.push(Edge {
                id: stable_id(
                    "edge",
                    [&package_id, &function.id, &parameter.id, "HAS_PARAMETER"],
                ),
                package_id: package_id.clone(),
                from_id: function.id.clone(),
                to_id: parameter.id.clone(),
                edge_type: EdgeType::HasParameter,
                operation_id: None,
                source_span: function.source_span.clone(),
                metadata_json: Some(json!({
                    "name": parameter.name,
                    "type": parameter.type_name,
                    "index": parameter.index,
                })),
            });
            let type_target = resolve_type_target(program, &parameter.type_name);
            program.edges.push(Edge {
                id: stable_id(
                    "edge",
                    [
                        &package_id,
                        &function.id,
                        &type_target,
                        &parameter.index.to_string(),
                        "ACCEPTS_TYPE",
                    ],
                ),
                package_id: package_id.clone(),
                from_id: function.id.clone(),
                to_id: type_target,
                edge_type: EdgeType::AcceptsType,
                operation_id: None,
                source_span: function.source_span.clone(),
                metadata_json: Some(json!({ "type": parameter.type_name })),
            });
        }
        for (index, return_type) in function.returns.iter().enumerate() {
            let type_target = resolve_type_target(program, return_type);
            program.edges.push(Edge {
                id: stable_id(
                    "edge",
                    [
                        &package_id,
                        &function.id,
                        &type_target,
                        &index.to_string(),
                        "RETURNS_TYPE",
                    ],
                ),
                package_id: package_id.clone(),
                from_id: function.id.clone(),
                to_id: type_target,
                edge_type: EdgeType::ReturnsType,
                operation_id: None,
                source_span: function.source_span.clone(),
                metadata_json: Some(json!({ "type": return_type })),
            });
        }
        program.functions.push(function);
    }
}

fn resolve_type_target(program: &ProgramIndex, type_name: &str) -> String {
    program
        .types
        .iter()
        .find(|type_def| {
            type_name.contains(&type_def.full_name) || type_name.ends_with(&type_def.name)
        })
        .map(|type_def| type_def.id.clone())
        .unwrap_or_else(|| type_name.to_string())
}

fn type_from_summary(
    package_id: &str,
    module_id: &str,
    module_full_name: &str,
    name: &str,
    value: &Value,
    kind: TypeKind,
    span: SourceSpan,
) -> TypeDef {
    let full_name = format!("{module_full_name}::{name}");
    let type_id = logical_id("type", [package_id, &full_name]);
    let fields = summary_fields(value)
        .into_iter()
        .map(|(field_name, type_name)| FieldInfo {
            id: logical_id("field", [&type_id, &field_name]),
            package_id: package_id.to_string(),
            module_id: module_id.to_string(),
            type_id: type_id.clone(),
            name: field_name,
            type_name,
            source_span: span.clone(),
        })
        .collect::<Vec<_>>();

    TypeDef {
        id: type_id,
        package_id: package_id.to_string(),
        module_id: module_id.to_string(),
        name: name.to_string(),
        full_name,
        kind,
        abilities: abilities(value),
        type_parameters: type_parameters(value),
        fields,
        docs: string_field(value, "doc").or_else(|| string_field(value, "docs")),
        attributes: attributes(value),
        source_span: span,
    }
}

fn function_from_summary(
    package_id: &str,
    module_id: &str,
    module_full_name: &str,
    name: &str,
    value: &Value,
    span: SourceSpan,
) -> FunctionInfo {
    let full_name = format!("{module_full_name}::{name}");
    let function_id = logical_id("function", [package_id, &full_name]);
    let parameters = value
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, parameter)| FunctionParameter {
            id: logical_id("parameter", [&function_id, &index.to_string()]),
            name: parameter
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            type_name: parameter
                .get("type_")
                .or_else(|| parameter.get("type"))
                .map(type_value_to_string)
                .unwrap_or_else(|| type_value_to_string(parameter)),
            index,
        })
        .collect::<Vec<_>>();
    let returns = value
        .get("return_")
        .or_else(|| value.get("returns"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(type_value_to_string)
        .collect::<Vec<_>>();

    FunctionInfo {
        id: function_id,
        package_id: package_id.to_string(),
        module_id: module_id.to_string(),
        name: name.to_string(),
        full_name,
        visibility: visibility(value.get("visibility")),
        is_entry: value.get("entry").and_then(Value::as_bool).unwrap_or(false),
        is_native: value
            .get("is_native")
            .or_else(|| value.get("native"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_parameters: type_parameters(value),
        parameters,
        returns,
        acquires: value
            .get("acquires")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(type_value_to_string)
            .collect(),
        docs: string_field(value, "doc").or_else(|| string_field(value, "docs")),
        attributes: attributes(value),
        source_span: span,
    }
}

fn emit_function_tags(program: &mut ProgramIndex, function: &FunctionInfo) {
    let mut tags = Vec::new();
    if function.is_entry && function.visibility == FunctionVisibility::Public {
        tags.push("public_entry_detected");
    }
    if function.visibility == FunctionVisibility::Public {
        tags.push("public_function_detected");
    }
    if function.visibility == FunctionVisibility::PublicFriend {
        tags.push("friend_function_detected");
    }
    if function.is_native {
        tags.push("native_function_detected");
    }
    if function.name == "init" {
        tags.push("init_function_detected");
    }
    if !function.type_parameters.is_empty() {
        tags.push("generic_function_detected");
    }
    if function
        .parameters
        .iter()
        .any(|parameter| parameter.type_name.contains("TxContext"))
    {
        tags.push("tx_context_parameter_detected");
    }
    if function
        .parameters
        .iter()
        .any(|parameter| parameter.type_name.contains("&mut"))
    {
        tags.push("mutable_reference_parameter_detected");
    }
    for tag in tags {
        push_tag(program, &function.id, tag, function.source_span.clone());
    }
}

fn emit_type_tags(program: &mut ProgramIndex, type_def: &TypeDef) {
    if type_def
        .abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("key"))
    {
        push_tag(
            program,
            &type_def.id,
            "ability_key_detected",
            type_def.source_span.clone(),
        );
    }
    if type_def
        .abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("store"))
    {
        push_tag(
            program,
            &type_def.id,
            "store_type_detected",
            type_def.source_span.clone(),
        );
    }
    if type_def
        .fields
        .iter()
        .any(|field| field.name == "id" && field.type_name.contains("UID"))
    {
        push_tag(
            program,
            &type_def.id,
            "uid_field_detected",
            type_def.source_span.clone(),
        );
        push_tag(
            program,
            &type_def.id,
            "key_object_type_detected",
            type_def.source_span.clone(),
        );
    }
    if type_def.name.to_ascii_lowercase().contains("cap") {
        push_tag(
            program,
            &type_def.id,
            "capability_named_type_detected",
            type_def.source_span.clone(),
        );
    }
    if type_def.name.to_ascii_lowercase().contains("witness") {
        push_tag(
            program,
            &type_def.id,
            "witness_named_type_detected",
            type_def.source_span.clone(),
        );
    }
}

fn push_tag(program: &mut ProgramIndex, target_id: &str, tag: &str, source_span: SourceSpan) {
    if !is_neutral_tag(tag) {
        return;
    }
    program.semantic_tags.push(SemanticTag {
        id: stable_id("tag", [&program.package.id, target_id, tag]),
        package_id: program.package.id.clone(),
        target_id: target_id.to_string(),
        tag: tag.to_string(),
        source_span,
        metadata_json: None,
    });
}

fn module_card_json(summary: &SummaryModule, dependency: bool, symbol_name: Option<&str>) -> Value {
    let public_symbols = summary
        .functions
        .iter()
        .filter(|(name, value)| {
            symbol_name.is_none_or(|target| *name == target)
                && matches!(
                    visibility(value.get("visibility")),
                    FunctionVisibility::Public
                        | FunctionVisibility::PublicFriend
                        | FunctionVisibility::PublicPackage
                )
        })
        .take(if dependency { 24 } else { usize::MAX })
        .map(|(name, value)| {
            json!({
                "name": name,
                "kind": "function",
                "visibility": format!("{:?}", visibility(value.get("visibility"))),
                "entry": value.get("entry").and_then(Value::as_bool).unwrap_or(false),
                "signature": compact_signature(name, value),
            })
        })
        .collect::<Vec<_>>();

    let types = summary
        .structs
        .iter()
        .filter(|(name, _)| symbol_name.is_none_or(|target| *name == target))
        .take(if dependency { 16 } else { usize::MAX })
        .map(|(name, value)| {
            json!({
                "name": name,
                "kind": "struct",
                "abilities": abilities(value),
                "field_count": summary_fields(value).len(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "package_alias": summary.id.address,
        "module_name": summary.id.name,
        "immediate_dependencies": summary.immediate_dependencies.iter().map(|dep| format!("{}::{}", dep.address, dep.name)).collect::<Vec<_>>(),
        "public_functions_count": summary.functions.values().filter(|value| matches!(visibility(value.get("visibility")), FunctionVisibility::Public | FunctionVisibility::PublicFriend | FunctionVisibility::PublicPackage)).count(),
        "types_count": summary.structs.len() + summary.enums.len(),
        "selected_public_symbols": public_symbols,
        "types": types,
    })
}

fn compact_signature(name: &str, value: &Value) -> String {
    let params = value
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|parameter| {
            parameter
                .get("type_")
                .or_else(|| parameter.get("type"))
                .map(type_value_to_string)
                .unwrap_or_else(|| type_value_to_string(parameter))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let returns = value
        .get("return_")
        .or_else(|| value.get("returns"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(type_value_to_string)
        .collect::<Vec<_>>();
    if returns.is_empty() {
        format!("{name}({params})")
    } else {
        format!("{name}({params}): {}", returns.join(", "))
    }
}

fn summary_artifact(
    package_id: &str,
    package_alias: String,
    module_name: String,
    path: &Path,
    content_hash: String,
    role: PackageRole,
    materialized_status: MaterializedStatus,
    card_json: Option<Value>,
    last_seen_at: i64,
) -> SummaryArtifact {
    SummaryArtifact {
        id: summary_artifact_id(package_id, &package_alias, &module_name, &content_hash),
        package_id: package_id.to_string(),
        package_alias,
        module_name,
        summary_path: path.to_string_lossy().into_owned(),
        content_hash,
        schema_version: None,
        role,
        materialized_status,
        last_seen_at,
        card_json,
    }
}

fn summary_artifact_id(
    package_id: &str,
    package_alias: &str,
    module_name: &str,
    content_hash: &str,
) -> String {
    let short_hash = content_hash.get(..16).unwrap_or(content_hash);
    logical_id(
        "summary",
        [package_id, package_alias, module_name, short_hash],
    )
}

fn read_address_mapping(path: &Path, package_id: &str) -> Vec<AddressMapping> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(mapping) = serde_json::from_str::<BTreeMap<String, String>>(&source) else {
        return Vec::new();
    };
    mapping
        .into_iter()
        .map(|(alias, address)| AddressMapping {
            id: stable_id("address_mapping", [package_id, &alias, &address]),
            package_id: package_id.to_string(),
            alias,
            address,
        })
        .collect()
}

fn read_compact_json(path: &Path) -> Option<Value> {
    let source = fs::read_to_string(path).ok()?;
    serde_json::from_str(&source).ok()
}

fn derived_summary_identity(summary_root: Option<&Path>, path: &Path) -> (String, String) {
    let module_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    let package_alias = summary_root
        .and_then(|root| path.strip_prefix(root).ok())
        .and_then(|relative| relative.components().next())
        .and_then(|component| component.as_os_str().to_str())
        .unwrap_or("unknown")
        .to_string();
    (package_alias, module_name)
}

fn classify_package_role(alias: &str, root_alias: &str) -> PackageRole {
    if alias == root_alias {
        PackageRole::Root
    } else if matches!(alias, "sui" | "std" | "move_stdlib") {
        PackageRole::Framework
    } else if alias.contains("pyth") {
        PackageRole::OracleDependency
    } else if alias.contains("coin") || alias.contains("token") {
        PackageRole::TokenDependency
    } else if alias.contains("deepbook") || alias.contains("navi") || alias.contains("margin") {
        PackageRole::ProtocolDependency
    } else {
        PackageRole::UnknownDependency
    }
}

fn visibility(value: Option<&Value>) -> FunctionVisibility {
    match value {
        Some(Value::String(value)) if value.contains("Friend") => FunctionVisibility::PublicFriend,
        Some(Value::String(value)) if value.contains("Package") => {
            FunctionVisibility::PublicPackage
        }
        Some(Value::String(value)) if value.contains("Public") => FunctionVisibility::Public,
        Some(Value::String(value)) if value.contains("Native") => FunctionVisibility::Native,
        Some(Value::Object(value)) if value.contains_key("Public") => FunctionVisibility::Public,
        Some(Value::Object(value)) if value.contains_key("Friend") => {
            FunctionVisibility::PublicFriend
        }
        Some(Value::Object(value)) if value.contains_key("Package") => {
            FunctionVisibility::PublicPackage
        }
        Some(Value::Object(value)) if value.contains_key("Native") => FunctionVisibility::Native,
        _ => FunctionVisibility::Private,
    }
}

fn type_parameters(value: &Value) -> Vec<String> {
    value
        .get("type_parameters")
        .or_else(|| value.get("type_params"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(type_value_to_string)
        .collect()
}

fn abilities(value: &Value) -> Vec<String> {
    let mut result = value
        .get("abilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(type_value_to_string)
        .map(|ability| ability.to_ascii_lowercase())
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}

fn attributes(value: &Value) -> Vec<String> {
    value
        .get("attributes")
        .map(value_array_strings)
        .unwrap_or_default()
}

fn value_array_strings(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items.iter().map(type_value_to_string).collect(),
        Value::String(value) => vec![value.clone()],
        Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn summary_fields(value: &Value) -> Vec<(String, String)> {
    let fields = value
        .get("fields")
        .and_then(|fields| fields.get("fields").or(Some(fields)))
        .and_then(Value::as_object);
    let mut result = fields
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter_map(|(field_name, field)| {
            field
                .get("type_")
                .or_else(|| field.get("type"))
                .map(type_value_to_string)
                .map(|type_name| (field_name.clone(), type_name))
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result
}

fn type_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Object(map) => {
            if let Some(value) = map.get("Reference") {
                return reference_type_to_string(value);
            }
            if let Some(value) = map.get("NamedTypeParameter").and_then(Value::as_str) {
                return value.to_string();
            }
            if let Some(value) = map.get("argument") {
                return type_value_to_string(value);
            }
            if let Some(value) = map.get("Datatype").or_else(|| map.get("Struct")) {
                return type_value_to_string(value);
            }
            if let Some(value) = map.get("MutableReference") {
                return format!("&mut {}", type_value_to_string(value));
            }
            if let Some(value) = map.get("Vector").or_else(|| map.get("vector")) {
                return format!("vector<{}>", type_value_to_string(value));
            }
            if let Some(name) = map.get("name").and_then(Value::as_str) {
                if let Some(constraints) = constraints_to_string(map.get("constraints")) {
                    let name = if map.get("phantom").and_then(Value::as_bool).unwrap_or(false) {
                        format!("phantom {name}")
                    } else {
                        name.to_string()
                    };
                    return format!("{name}: {constraints}");
                }
                if let Some(module) = datatype_module_to_string(map.get("module")) {
                    let mut rendered = format!("{module}::{name}");
                    if let Some(arguments) = type_arguments_to_string(map.get("type_arguments")) {
                        rendered.push('<');
                        rendered.push_str(&arguments);
                        rendered.push('>');
                    }
                    return rendered;
                }
                return name.to_string();
            }
            serde_json::to_string(value).unwrap_or_default()
        }
        Value::Array(values) => values
            .iter()
            .map(type_value_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "unknown".to_string(),
    }
}

fn constraints_to_string(value: Option<&Value>) -> Option<String> {
    let constraints = value?
        .as_array()?
        .iter()
        .map(type_value_to_string)
        .map(|constraint| constraint.to_ascii_lowercase())
        .collect::<Vec<_>>();
    (!constraints.is_empty()).then(|| constraints.join(" + "))
}

fn type_arguments_to_string(value: Option<&Value>) -> Option<String> {
    let arguments = value?
        .as_array()?
        .iter()
        .map(type_value_to_string)
        .collect::<Vec<_>>();
    (!arguments.is_empty()).then(|| arguments.join(", "))
}

fn reference_type_to_string(value: &Value) -> String {
    match value {
        Value::Array(items) if items.len() == 2 => {
            let mutable = items.first().and_then(Value::as_bool).unwrap_or(false);
            let inner = items
                .get(1)
                .map(type_value_to_string)
                .unwrap_or_else(|| "unknown".to_string());
            if mutable {
                format!("&mut {inner}")
            } else {
                format!("&{inner}")
            }
        }
        _ => format!("&{}", type_value_to_string(value)),
    }
}

fn datatype_module_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(module)) => Some(module.clone()),
        Some(Value::Object(module)) => {
            let address = module.get("address").and_then(Value::as_str)?;
            let name = module.get("name").and_then(Value::as_str)?;
            Some(format!("{address}::{name}"))
        }
        _ => None,
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
