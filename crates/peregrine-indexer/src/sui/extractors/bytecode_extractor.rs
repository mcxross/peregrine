use std::{collections::BTreeMap, fs, path::Path};

use move_binary_format::file_format::{
    Bytecode, CodeOffset, CompiledModule, FieldHandleIndex, FieldInstantiationIndex,
    FunctionHandleIndex, SignatureToken, StructDefinitionIndex, StructFieldInformation,
};
use walkdir::WalkDir;

use crate::{
    core::{
        BasicBlock, Edge, EdgeType, LocalInfo, Operation, OperationKind, SemanticTag, SourceSpan,
        logical_id, stable_id,
    },
    sui::{model::ProgramIndex, source_map::SourceMapIndex},
};

pub fn enrich_program_from_build(program: &mut ProgramIndex, build_root: &Path) {
    let modules_root = build_root.join("bytecode_modules");
    if !modules_root.is_dir() {
        return;
    }

    let mut function_ids = BTreeMap::new();
    let mut module_ids = BTreeMap::new();
    for function in &program.functions {
        if let Some(module) = program
            .modules
            .iter()
            .find(|module| module.id == function.module_id)
        {
            module_ids.insert(module.name.clone(), module.id.clone());
            function_ids.insert(
                (module.name.clone(), function.name.clone()),
                function.id.clone(),
            );
        }
    }
    let mut field_ids = BTreeMap::new();
    let mut type_ids = BTreeMap::new();
    for type_def in &program.types {
        if let Some(module) = program
            .modules
            .iter()
            .find(|module| module.id == type_def.module_id)
        {
            type_ids.insert(
                (module.name.clone(), type_def.name.clone()),
                type_def.id.clone(),
            );
            for field in &type_def.fields {
                field_ids.insert(
                    (
                        module.name.clone(),
                        type_def.name.clone(),
                        field.name.clone(),
                    ),
                    field.id.clone(),
                );
            }
        }
    }
    let source_maps = SourceMapIndex::load(program, build_root);
    let function_spans = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function.source_span.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut paths = WalkDir::new(modules_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mv"))
        .filter(|path| {
            !path
                .components()
                .any(|component| component.as_os_str().to_str() == Some("dependencies"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(module) = CompiledModule::deserialize_with_defaults(&bytes) else {
            continue;
        };
        let module_name = module.self_id().name().to_string();
        if let Some(source_module_id) = module_ids.get(&module_name).cloned() {
            let source_span = program
                .modules
                .iter()
                .find(|indexed_module| indexed_module.id == source_module_id)
                .map(|indexed_module| indexed_module.source_span.clone())
                .unwrap_or_else(SourceSpan::unknown);
            for friend in module.immediate_friends() {
                let friend_name = friend.name().to_string();
                let target = module_ids.get(&friend_name).cloned().unwrap_or_else(|| {
                    format!(
                        "{}::{}",
                        friend.address().short_str_lossless(),
                        friend.name()
                    )
                });
                program.edges.push(Edge {
                    id: stable_id(
                        "edge",
                        [&program.package.id, &source_module_id, &target, "FRIENDS"],
                    ),
                    package_id: program.package.id.clone(),
                    from_id: source_module_id.clone(),
                    to_id: target,
                    edge_type: EdgeType::Friends,
                    operation_id: None,
                    source_span: source_span.clone(),
                    metadata_json: Some(serde_json::json!({
                        "source": "compiled_module.friend_decls"
                    })),
                });
            }
        }
        for (function_index, definition) in module.function_defs().iter().enumerate() {
            let handle = module.function_handle_at(definition.function);
            let function_name = module.identifier_at(handle.name).to_string();
            let Some(function_id) = function_ids
                .get(&(module_name.clone(), function_name.clone()))
                .cloned()
            else {
                continue;
            };
            let source_span = source_maps
                .function_span(&module_name, function_index)
                .or_else(|| function_spans.get(&function_id).cloned())
                .unwrap_or_else(SourceSpan::unknown);
            if let Some(function) = program
                .functions
                .iter_mut()
                .find(|function| function.id == function_id)
            {
                function.source_span = source_span.clone();
            }
            let Some(code) = &definition.code else {
                continue;
            };
            let parameter_count = module.signature_at(handle.parameters).0.len();
            for (local_index, token) in module.signature_at(code.locals).0.iter().enumerate() {
                let source_local_index = (parameter_count + local_index) as u64;
                let (name, local_span) = source_maps
                    .local_name_and_span(&module_name, function_index, source_local_index)
                    .unwrap_or_else(|| (format!("local_{local_index}"), source_span.clone()));
                program.locals.push(LocalInfo {
                    id: logical_id("local", [&function_id, &local_index.to_string()]),
                    package_id: program.package.id.clone(),
                    function_id: function_id.clone(),
                    name,
                    type_name: Some(signature_token_label(&module, token)),
                    index_in_function: Some(local_index),
                    source_span: local_span,
                });
            }
            let basic_blocks = build_basic_blocks(
                &program.package.id,
                &function_id,
                &code.code,
                &code.jump_tables,
                &source_span,
            );
            for edge in control_flow_edges(
                &program.package.id,
                &function_id,
                &code.code,
                &code.jump_tables,
                &basic_blocks,
                &source_span,
            ) {
                program.edges.push(edge);
            }
            program.basic_blocks.extend(basic_blocks);

            let mut pending_field_reference: Option<(String, String, bool, String)> = None;
            for (index, bytecode) in code.code.iter().enumerate() {
                let (mut kind, mut display, target) = operation_parts(&module, bytecode);
                let operation_span = source_maps
                    .operation_span(&module_name, function_index, index)
                    .unwrap_or_else(|| source_span.clone());
                if is_assert_pattern(&code.code, index, &code.jump_tables)
                    && source_confirms_assert(&source_maps, &operation_span, &source_span)
                {
                    kind = OperationKind::Assert;
                    display = format!("assert-pattern {bytecode:?}");
                }
                let operation_id = logical_id("operation", [&function_id, &index.to_string()]);
                program.operations.push(Operation {
                    id: operation_id.clone(),
                    package_id: program.package.id.clone(),
                    function_id: function_id.clone(),
                    index_in_function: index,
                    kind: kind.clone(),
                    display,
                    target: target.clone(),
                    source_span: operation_span.clone(),
                    metadata_json: Some(serde_json::json!({
                        "bytecode": format!("{bytecode:?}"),
                        "offset": index,
                        "source_map_precision": format!("{:?}", operation_span.precision),
                    })),
                });
                if kind == OperationKind::Call {
                    if let Some(target) = target.as_deref() {
                        let edge_target = resolve_call_edge_target(&function_ids, target)
                            .unwrap_or_else(|| target.to_string());
                        program.edges.push(Edge {
                            id: stable_id(
                                "edge",
                                [
                                    &program.package.id,
                                    &function_id,
                                    &edge_target,
                                    &operation_id,
                                    "CALLS",
                                ],
                            ),
                            package_id: program.package.id.clone(),
                            from_id: function_id.clone(),
                            to_id: edge_target,
                            edge_type: EdgeType::Calls,
                            operation_id: Some(operation_id.clone()),
                            source_span: operation_span.clone(),
                            metadata_json: Some(serde_json::json!({ "target": target })),
                        });
                        emit_call_tag(program, &operation_id, target, operation_span.clone());
                    }
                }
                if matches!(kind, OperationKind::Pack | OperationKind::Unpack) {
                    if let Some(target) = target.as_deref() {
                        let edge_target = resolve_type_edge_target(&type_ids, target)
                            .unwrap_or_else(|| target.to_string());
                        let edge_type = if kind == OperationKind::Pack {
                            EdgeType::PacksType
                        } else {
                            EdgeType::UnpacksType
                        };
                        let edge_kind = if kind == OperationKind::Pack {
                            "PACKS_TYPE"
                        } else {
                            "UNPACKS_TYPE"
                        };
                        program.edges.push(Edge {
                            id: stable_id(
                                "edge",
                                [
                                    program.package.id.as_str(),
                                    function_id.as_str(),
                                    edge_target.as_str(),
                                    operation_id.as_str(),
                                    edge_kind,
                                ],
                            ),
                            package_id: program.package.id.clone(),
                            from_id: function_id.clone(),
                            to_id: edge_target,
                            edge_type,
                            operation_id: Some(operation_id.clone()),
                            source_span: operation_span.clone(),
                            metadata_json: Some(serde_json::json!({ "target": target })),
                        });
                    }
                }
                if let Some((field_target, is_mutable)) = field_borrow_target(&module, bytecode) {
                    let edge_target = resolve_field_edge_target(&field_ids, &field_target)
                        .unwrap_or_else(|| field_target.clone());
                    let edge_kind = if is_mutable {
                        "BORROWS_FIELD_MUT"
                    } else {
                        "BORROWS_FIELD"
                    };
                    program.edges.push(Edge {
                        id: stable_id(
                            "edge",
                            [
                                &program.package.id,
                                &function_id,
                                &edge_target,
                                &operation_id,
                                edge_kind,
                            ],
                        ),
                        package_id: program.package.id.clone(),
                        from_id: function_id.clone(),
                        to_id: edge_target.clone(),
                        edge_type: if is_mutable {
                            EdgeType::BorrowsFieldMut
                        } else {
                            EdgeType::BorrowsField
                        },
                        operation_id: Some(operation_id.clone()),
                        source_span: operation_span.clone(),
                        metadata_json: Some(serde_json::json!({
                            "field": field_target,
                            "bytecode": format!("{bytecode:?}")
                        })),
                    });
                    pending_field_reference =
                        Some((field_target, edge_target, is_mutable, operation_id.clone()));
                }
                match bytecode {
                    Bytecode::ReadRef => {
                        if let Some((field_target, edge_target, _, borrow_operation_id)) =
                            pending_field_reference.clone()
                        {
                            program.edges.push(Edge {
                                id: stable_id(
                                    "edge",
                                    [
                                        &program.package.id,
                                        &function_id,
                                        &edge_target,
                                        &operation_id,
                                        "READS_FIELD",
                                    ],
                                ),
                                package_id: program.package.id.clone(),
                                from_id: function_id.clone(),
                                to_id: edge_target,
                                edge_type: EdgeType::ReadsField,
                                operation_id: Some(operation_id.clone()),
                                source_span: operation_span.clone(),
                                metadata_json: Some(serde_json::json!({
                                    "borrow_operation_id": borrow_operation_id,
                                    "field": field_target,
                                })),
                            });
                        }
                    }
                    Bytecode::WriteRef => {
                        if let Some((field_target, edge_target, _, borrow_operation_id)) =
                            pending_field_reference.clone()
                        {
                            program.edges.push(Edge {
                                id: stable_id(
                                    "edge",
                                    [
                                        &program.package.id,
                                        &function_id,
                                        &edge_target,
                                        &operation_id,
                                        "WRITES_FIELD",
                                    ],
                                ),
                                package_id: program.package.id.clone(),
                                from_id: function_id.clone(),
                                to_id: edge_target,
                                edge_type: EdgeType::WritesField,
                                operation_id: Some(operation_id.clone()),
                                source_span: operation_span.clone(),
                                metadata_json: Some(serde_json::json!({
                                    "borrow_operation_id": borrow_operation_id,
                                    "field": field_target,
                                })),
                            });
                        }
                    }
                    _ if !matches!(
                        bytecode,
                        Bytecode::MutBorrowField(_)
                            | Bytecode::MutBorrowFieldGeneric(_)
                            | Bytecode::ImmBorrowField(_)
                            | Bytecode::ImmBorrowFieldGeneric(_)
                    ) =>
                    {
                        if !matches!(bytecode, Bytecode::ReadRef | Bytecode::WriteRef) {
                            pending_field_reference = None;
                        }
                    }
                    _ => {}
                }
                emit_operation_tag(program, &operation_id, &kind, operation_span);
            }
        }
    }
}

fn build_basic_blocks(
    package_id: &str,
    function_id: &str,
    code: &[Bytecode],
    jump_tables: &[move_binary_format::file_format::VariantJumpTable],
    source_span: &SourceSpan,
) -> Vec<BasicBlock> {
    if code.is_empty() {
        return Vec::new();
    }

    let mut leaders = BTreeMap::<CodeOffset, ()>::new();
    leaders.insert(0, ());
    for (offset, instruction) in code.iter().enumerate() {
        let offset = offset as CodeOffset;
        for target in instruction.offsets(jump_tables) {
            if (target as usize) < code.len() {
                leaders.insert(target, ());
            }
        }
        if instruction.is_branch() {
            let next = offset.saturating_add(1);
            if (next as usize) < code.len() {
                leaders.insert(next, ());
            }
        }
    }
    let starts = leaders.keys().copied().collect::<Vec<_>>();
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts
                .get(index + 1)
                .copied()
                .map(|next| next.saturating_sub(1))
                .unwrap_or_else(|| code.len().saturating_sub(1) as CodeOffset);
            BasicBlock {
                id: logical_id("block", [function_id, &index.to_string()]),
                package_id: package_id.to_string(),
                function_id: function_id.to_string(),
                index_in_function: index,
                label: if index == 0 {
                    "BB0 (entry)".to_string()
                } else {
                    format!("BB{index}")
                },
                start_operation_index: Some(*start as usize),
                end_operation_index: Some(end as usize),
                source_span: source_span.clone(),
            }
        })
        .collect()
}

fn control_flow_edges(
    package_id: &str,
    function_id: &str,
    code: &[Bytecode],
    jump_tables: &[move_binary_format::file_format::VariantJumpTable],
    blocks: &[BasicBlock],
    source_span: &SourceSpan,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for block in blocks {
        let Some(end) = block.end_operation_index else {
            continue;
        };
        let Some(instruction) = code.get(end) else {
            continue;
        };
        let successors = Bytecode::get_successors(end as CodeOffset, code, jump_tables);
        for successor in successors {
            let Some(target_block) = blocks.iter().find(|candidate| {
                candidate
                    .start_operation_index
                    .zip(candidate.end_operation_index)
                    .is_some_and(|(start, end)| {
                        successor as usize >= start && successor as usize <= end
                    })
            }) else {
                continue;
            };
            edges.push(Edge {
                id: stable_id(
                    "edge",
                    [
                        package_id,
                        function_id,
                        &block.id,
                        &target_block.id,
                        &successor.to_string(),
                        "CONTROL_FLOW",
                    ],
                ),
                package_id: package_id.to_string(),
                from_id: block.id.clone(),
                to_id: target_block.id.clone(),
                edge_type: EdgeType::ControlFlow,
                operation_id: Some(logical_id("operation", [function_id, &end.to_string()])),
                source_span: source_span.clone(),
                metadata_json: Some(serde_json::json!({
                    "source_offset": end,
                    "target_offset": successor,
                    "branch": format!("{instruction:?}")
                })),
            });
        }
    }
    edges
}

fn operation_parts(
    module: &CompiledModule,
    bytecode: &Bytecode,
) -> (OperationKind, String, Option<String>) {
    match bytecode {
        Bytecode::Nop => (OperationKind::Nop, "nop".to_string(), None),
        Bytecode::LdU8(_)
        | Bytecode::LdU16(_)
        | Bytecode::LdU32(_)
        | Bytecode::LdU64(_)
        | Bytecode::LdU128(_)
        | Bytecode::LdU256(_)
        | Bytecode::LdConst(_)
        | Bytecode::LdTrue
        | Bytecode::LdFalse => (OperationKind::Constant, format!("{bytecode:?}"), None),
        Bytecode::CopyLoc(_) => (OperationKind::Copy, format!("{bytecode:?}"), None),
        Bytecode::MoveLoc(_) => (OperationKind::Move, format!("{bytecode:?}"), None),
        Bytecode::StLoc(_) => (OperationKind::Assign, format!("{bytecode:?}"), None),
        Bytecode::Call(handle) => {
            let target = call_target(module, *handle);
            (
                OperationKind::Call,
                format!("call {}", target.as_deref().unwrap_or("<unknown>")),
                target,
            )
        }
        Bytecode::CallGeneric(index) => {
            let instantiation = module.function_instantiation_at(*index);
            let target = call_target(module, instantiation.handle);
            (
                OperationKind::Call,
                format!("call {}", target.as_deref().unwrap_or("<unknown>")),
                target,
            )
        }
        Bytecode::Ret => (OperationKind::Return, "return".to_string(), None),
        Bytecode::Abort => (OperationKind::Abort, "abort".to_string(), None),
        Bytecode::Branch(_) => (OperationKind::Branch, format!("{bytecode:?}"), None),
        Bytecode::BrTrue(_) | Bytecode::BrFalse(_) | Bytecode::VariantSwitch(_) => {
            (OperationKind::BranchIf, format!("{bytecode:?}"), None)
        }
        Bytecode::Eq => (OperationKind::CompareEq, "eq".to_string(), None),
        Bytecode::Neq => (OperationKind::CompareNeq, "neq".to_string(), None),
        Bytecode::Lt => (OperationKind::CompareLt, "lt".to_string(), None),
        Bytecode::Gt => (OperationKind::CompareGt, "gt".to_string(), None),
        Bytecode::Le => (OperationKind::CompareLe, "le".to_string(), None),
        Bytecode::Ge => (OperationKind::CompareGe, "ge".to_string(), None),
        Bytecode::Add => (OperationKind::Add, "add".to_string(), None),
        Bytecode::Sub => (OperationKind::Sub, "sub".to_string(), None),
        Bytecode::Mul => (OperationKind::Mul, "mul".to_string(), None),
        Bytecode::Div => (OperationKind::Div, "div".to_string(), None),
        Bytecode::Mod => (OperationKind::Mod, "mod".to_string(), None),
        Bytecode::MutBorrowLoc(_) | Bytecode::ImmBorrowLoc(_) => {
            (OperationKind::BorrowLocal, format!("{bytecode:?}"), None)
        }
        Bytecode::MutBorrowField(_) | Bytecode::MutBorrowFieldGeneric(_) => {
            (OperationKind::BorrowFieldMut, format!("{bytecode:?}"), None)
        }
        Bytecode::ImmBorrowField(_) | Bytecode::ImmBorrowFieldGeneric(_) => {
            (OperationKind::BorrowField, format!("{bytecode:?}"), None)
        }
        Bytecode::ReadRef => (OperationKind::ReadField, "read_ref".to_string(), None),
        Bytecode::WriteRef => (OperationKind::WriteField, "write_ref".to_string(), None),
        Bytecode::Pack(index) => (
            OperationKind::Pack,
            format!("pack {}", struct_name(module, *index).unwrap_or_default()),
            struct_name(module, *index),
        ),
        Bytecode::PackGeneric(index) => {
            let instantiation = module.struct_instantiation_at(*index);
            let target = struct_name(module, instantiation.def);
            (
                OperationKind::Pack,
                format!("pack {}", target.as_deref().unwrap_or_default()),
                target,
            )
        }
        Bytecode::Unpack(index) => (
            OperationKind::Unpack,
            format!("unpack {}", struct_name(module, *index).unwrap_or_default()),
            struct_name(module, *index),
        ),
        Bytecode::UnpackGeneric(index) => {
            let instantiation = module.struct_instantiation_at(*index);
            let target = struct_name(module, instantiation.def);
            (
                OperationKind::Unpack,
                format!("unpack {}", target.as_deref().unwrap_or_default()),
                target,
            )
        }
        Bytecode::FreezeRef => (OperationKind::FreezeRef, "freeze_ref".to_string(), None),
        Bytecode::MoveFromDeprecated(_) | Bytecode::MoveFromGenericDeprecated(_) => {
            (OperationKind::MoveFrom, format!("{bytecode:?}"), None)
        }
        Bytecode::MoveToDeprecated(_) | Bytecode::MoveToGenericDeprecated(_) => {
            (OperationKind::MoveTo, format!("{bytecode:?}"), None)
        }
        Bytecode::MutBorrowGlobalDeprecated(_) | Bytecode::MutBorrowGlobalGenericDeprecated(_) => (
            OperationKind::BorrowGlobalMut,
            format!("{bytecode:?}"),
            None,
        ),
        Bytecode::ImmBorrowGlobalDeprecated(_) | Bytecode::ImmBorrowGlobalGenericDeprecated(_) => {
            (OperationKind::BorrowGlobal, format!("{bytecode:?}"), None)
        }
        Bytecode::VecPack(_, _)
        | Bytecode::VecLen(_)
        | Bytecode::VecImmBorrow(_)
        | Bytecode::VecMutBorrow(_)
        | Bytecode::VecPushBack(_)
        | Bytecode::VecPopBack(_)
        | Bytecode::VecUnpack(_, _)
        | Bytecode::VecSwap(_) => (OperationKind::VectorOp, format!("{bytecode:?}"), None),
        Bytecode::PackVariant(_) | Bytecode::PackVariantGeneric(_) => {
            (OperationKind::Pack, format!("{bytecode:?}"), None)
        }
        Bytecode::UnpackVariant(_)
        | Bytecode::UnpackVariantImmRef(_)
        | Bytecode::UnpackVariantMutRef(_)
        | Bytecode::UnpackVariantGeneric(_)
        | Bytecode::UnpackVariantGenericImmRef(_)
        | Bytecode::UnpackVariantGenericMutRef(_) => {
            (OperationKind::Unpack, format!("{bytecode:?}"), None)
        }
        _ => (OperationKind::Unknown, format!("{bytecode:?}"), None),
    }
}

fn is_assert_pattern(
    code: &[Bytecode],
    index: usize,
    jump_tables: &[move_binary_format::file_format::VariantJumpTable],
) -> bool {
    let Some(Bytecode::BrTrue(target) | Bytecode::BrFalse(target)) = code.get(index) else {
        return false;
    };
    let target = *target as usize;
    if target <= index || target > code.len() {
        return false;
    }
    let branches_to_target = Bytecode::get_successors(index as CodeOffset, code, jump_tables)
        .contains(&(target as CodeOffset));
    branches_to_target
        && (code[index + 1..target]
            .iter()
            .any(|bytecode| matches!(bytecode, Bytecode::Abort))
            || abort_in_linear_block(code, target)
            || abort_in_linear_block(code, index.saturating_add(1)))
}

fn abort_in_linear_block(code: &[Bytecode], start: usize) -> bool {
    for bytecode in code.iter().skip(start) {
        match bytecode {
            Bytecode::Abort => return true,
            Bytecode::Ret
            | Bytecode::Branch(_)
            | Bytecode::BrTrue(_)
            | Bytecode::BrFalse(_)
            | Bytecode::VariantSwitch(_) => return false,
            _ => {}
        }
    }
    false
}

fn source_confirms_assert(
    source_maps: &SourceMapIndex,
    operation_span: &SourceSpan,
    function_span: &SourceSpan,
) -> bool {
    source_maps
        .source_text_for_span(operation_span)
        .or_else(|| source_maps.source_text_for_span(function_span))
        .map(|source| source.contains("assert!"))
        .unwrap_or(true)
}

fn call_target(module: &CompiledModule, handle_index: FunctionHandleIndex) -> Option<String> {
    let handle = module.function_handle_at(handle_index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    Some(format!(
        "{}::{}::{}",
        module_id.address().short_str_lossless(),
        module_id.name(),
        module.identifier_at(handle.name)
    ))
}

fn resolve_call_edge_target(
    function_ids: &BTreeMap<(String, String), String>,
    target: &str,
) -> Option<String> {
    let parts = target.split("::").collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    let module_name = parts.get(parts.len().saturating_sub(2))?;
    let function_name = parts.last()?;
    function_ids
        .get(&((*module_name).to_string(), (*function_name).to_string()))
        .cloned()
}

fn resolve_type_edge_target(
    type_ids: &BTreeMap<(String, String), String>,
    target: &str,
) -> Option<String> {
    let parts = target.split("::").collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    let module_name = parts.get(parts.len().saturating_sub(2))?;
    let type_name = parts.last()?;
    type_ids
        .get(&((*module_name).to_string(), (*type_name).to_string()))
        .cloned()
}

fn struct_name(module: &CompiledModule, index: StructDefinitionIndex) -> Option<String> {
    let definition = module.struct_def_at(index);
    let handle = module.datatype_handle_at(definition.struct_handle);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    Some(format!(
        "{}::{}::{}",
        module_id.address().short_str_lossless(),
        module_id.name(),
        module.identifier_at(handle.name)
    ))
}

fn field_borrow_target(module: &CompiledModule, bytecode: &Bytecode) -> Option<(String, bool)> {
    match bytecode {
        Bytecode::MutBorrowField(index) => {
            field_target(module, *index).map(|target| (target, true))
        }
        Bytecode::ImmBorrowField(index) => {
            field_target(module, *index).map(|target| (target, false))
        }
        Bytecode::MutBorrowFieldGeneric(index) => {
            generic_field_target(module, *index).map(|target| (target, true))
        }
        Bytecode::ImmBorrowFieldGeneric(index) => {
            generic_field_target(module, *index).map(|target| (target, false))
        }
        _ => None,
    }
}

fn generic_field_target(module: &CompiledModule, index: FieldInstantiationIndex) -> Option<String> {
    let instantiation = module.field_instantiation_at(index);
    field_target(module, instantiation.handle)
}

fn field_target(module: &CompiledModule, index: FieldHandleIndex) -> Option<String> {
    let handle = module.field_handle_at(index);
    let definition = module.struct_def_at(handle.owner);
    let owner = struct_name(module, handle.owner)?;
    let field_name = match &definition.field_information {
        StructFieldInformation::Declared(fields) => fields
            .get(handle.field as usize)
            .map(|field| module.identifier_at(field.name).to_string())
            .unwrap_or_else(|| handle.field.to_string()),
        StructFieldInformation::Native => handle.field.to_string(),
    };
    Some(format!("{owner}::{field_name}"))
}

fn resolve_field_edge_target(
    field_ids: &BTreeMap<(String, String, String), String>,
    target: &str,
) -> Option<String> {
    let parts = target.split("::").collect::<Vec<_>>();
    if parts.len() < 4 {
        return None;
    }
    let module_name = parts.get(parts.len().saturating_sub(3))?;
    let type_name = parts.get(parts.len().saturating_sub(2))?;
    let field_name = parts.last()?;
    field_ids
        .get(&(
            (*module_name).to_string(),
            (*type_name).to_string(),
            (*field_name).to_string(),
        ))
        .cloned()
}

fn signature_token_label(module: &CompiledModule, token: &SignatureToken) -> String {
    match token {
        SignatureToken::Bool => "bool".to_string(),
        SignatureToken::U8 => "u8".to_string(),
        SignatureToken::U16 => "u16".to_string(),
        SignatureToken::U32 => "u32".to_string(),
        SignatureToken::U64 => "u64".to_string(),
        SignatureToken::U128 => "u128".to_string(),
        SignatureToken::U256 => "u256".to_string(),
        SignatureToken::Address => "address".to_string(),
        SignatureToken::Signer => "signer".to_string(),
        SignatureToken::Vector(inner) => {
            format!("vector<{}>", signature_token_label(module, inner))
        }
        SignatureToken::Reference(inner) => format!("&{}", signature_token_label(module, inner)),
        SignatureToken::MutableReference(inner) => {
            format!("&mut {}", signature_token_label(module, inner))
        }
        SignatureToken::TypeParameter(index) => format!("T{index}"),
        SignatureToken::Datatype(index) => datatype_label(module, *index),
        SignatureToken::DatatypeInstantiation(instantiation) => {
            let (index, arguments) = &**instantiation;
            let arguments = arguments
                .iter()
                .map(|argument| signature_token_label(module, argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", datatype_label(module, *index))
        }
    }
}

fn datatype_label(
    module: &CompiledModule,
    index: move_binary_format::file_format::DatatypeHandleIndex,
) -> String {
    let handle = module.datatype_handle_at(index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    format!(
        "{}::{}::{}",
        module_id.address().short_str_lossless(),
        module_id.name(),
        module.identifier_at(handle.name)
    )
}

fn emit_operation_tag(
    program: &mut ProgramIndex,
    operation_id: &str,
    kind: &OperationKind,
    source_span: SourceSpan,
) {
    let tag = match kind {
        OperationKind::Abort => Some("abort_operation_detected"),
        OperationKind::Assert => Some("assert_operation_detected"),
        OperationKind::ReadField => Some("field_read_detected"),
        OperationKind::WriteField => Some("field_write_detected"),
        OperationKind::BorrowFieldMut | OperationKind::BorrowGlobalMut => {
            Some("mutable_borrow_detected")
        }
        OperationKind::Pack | OperationKind::CreateStruct => Some("object_creation_detected"),
        _ => None,
    };
    if let Some(tag) = tag {
        program.semantic_tags.push(SemanticTag {
            id: stable_id("tag", [&program.package.id, operation_id, tag]),
            package_id: program.package.id.clone(),
            target_id: operation_id.to_string(),
            tag: tag.to_string(),
            source_span,
            metadata_json: None,
        });
    }
}

fn emit_call_tag(
    program: &mut ProgramIndex,
    operation_id: &str,
    target: &str,
    source_span: SourceSpan,
) {
    let target_lower = target.to_ascii_lowercase();
    let tag = if target_lower.contains("::transfer::") {
        Some("transfer_api_call_detected")
    } else if target_lower.contains("::coin::") {
        Some("coin_api_call_detected")
    } else if target_lower.contains("::balance::") {
        Some("balance_api_call_detected")
    } else if target_lower.contains("::dynamic_field::") {
        Some("dynamic_field_api_call_detected")
    } else if target_lower.contains("::table::") {
        Some("table_api_call_detected")
    } else if target_lower.contains("::clock::") {
        Some("clock_api_call_detected")
    } else if target_lower.contains("::tx_context::") {
        Some("tx_context_api_call_detected")
    } else {
        Some("package_external_call_detected")
    };
    if let Some(tag) = tag {
        program.semantic_tags.push(SemanticTag {
            id: stable_id("tag", [&program.package.id, operation_id, tag]),
            package_id: program.package.id.clone(),
            target_id: operation_id.to_string(),
            tag: tag.to_string(),
            source_span,
            metadata_json: Some(serde_json::json!({ "target": target })),
        });
    }
}
