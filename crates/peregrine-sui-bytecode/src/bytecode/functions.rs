fn module_functions(
    module: &CompiledModule,
    source_map: Option<&SourceMap>,
) -> Vec<MoveBytecodeFunctionView> {
    module
        .function_defs()
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            let handle = module.function_handle_at(definition.function);
            let name = module.identifier_at(handle.name).to_string();
            let code = definition.code.as_ref();
            let instructions = code
                .map(|code| {
                    code.code
                        .iter()
                        .enumerate()
                        .map(|(offset, instruction)| {
                            instruction_view(
                                module,
                                instruction,
                                source_map,
                                FunctionDefinitionIndex(index as u16),
                                offset as CodeOffset,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let local_count = definition
                .code
                .as_ref()
                .map(|code| module.signature_at(code.locals).0.len())
                .unwrap_or(0);
            let return_count = module.signature_at(handle.return_).0.len();
            let parameters = module
                .signature_at(handle.parameters)
                .0
                .iter()
                .map(|token| signature_token_label(module, token))
                .collect();
            let returns = module
                .signature_at(handle.return_)
                .0
                .iter()
                .map(|token| signature_token_label(module, token))
                .collect();
            let acquires = definition
                .acquires_global_resources
                .iter()
                .map(|index| {
                    let definition = module.struct_def_at(*index);
                    let handle = module.datatype_handle_at(definition.struct_handle);
                    module.identifier_at(handle.name).to_string()
                })
                .collect();

            MoveBytecodeFunctionView {
                name,
                visibility: format!("{:?}", definition.visibility),
                is_entry: definition.is_entry,
                parameters,
                returns,
                type_parameter_count: handle.type_parameters.len(),
                instruction_count: instructions.len(),
                local_count,
                return_count,
                acquires,
                instructions,
                control_flow: code
                    .map(|code| build_control_flow(&code.code, &code.jump_tables))
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn build_control_flow(
    code: &[Bytecode],
    jump_tables: &[move_binary_format::file_format::VariantJumpTable],
) -> MoveBytecodeControlFlowView {
    if code.is_empty() {
        return MoveBytecodeControlFlowView::default();
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
    let blocks = starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts
                .get(index + 1)
                .copied()
                .map(|next_start| next_start.saturating_sub(1))
                .unwrap_or_else(|| code.len().saturating_sub(1) as CodeOffset);
            let instruction_offsets = (*start..=end).collect::<Vec<_>>();

            MoveBytecodeBasicBlockView {
                id: format!("bb-{index}"),
                label: if index == 0 {
                    "BB0 (entry)".to_string()
                } else {
                    format!("BB{index}")
                },
                start_offset: *start,
                end_offset: end,
                instruction_offsets,
            }
        })
        .collect::<Vec<_>>();

    let mut edges = Vec::new();

    for block in &blocks {
        let Some(last_instruction) = code.get(block.end_offset as usize) else {
            continue;
        };
        let branch_offsets = last_instruction.offsets(jump_tables);
        let successors = Bytecode::get_successors(block.end_offset, code, jump_tables);

        for successor in successors {
            let Some(target_block) = blocks.iter().find(|candidate| {
                successor >= candidate.start_offset && successor <= candidate.end_offset
            }) else {
                continue;
            };
            let kind = edge_kind(
                last_instruction,
                block.end_offset,
                successor,
                &branch_offsets,
            );

            edges.push(MoveBytecodeControlFlowEdgeView {
                source: block.id.clone(),
                target: target_block.id.clone(),
                source_offset: block.end_offset,
                target_offset: successor,
                kind,
            });
        }
    }

    MoveBytecodeControlFlowView { blocks, edges }
}

fn edge_kind(
    instruction: &Bytecode,
    source_offset: CodeOffset,
    target_offset: CodeOffset,
    branch_offsets: &[CodeOffset],
) -> String {
    match instruction {
        Bytecode::BrTrue(target) if *target == target_offset => "true".to_string(),
        Bytecode::BrTrue(_) if target_offset == source_offset.saturating_add(1) => {
            "false".to_string()
        }
        Bytecode::BrFalse(target) if *target == target_offset => "false".to_string(),
        Bytecode::BrFalse(_) if target_offset == source_offset.saturating_add(1) => {
            "true".to_string()
        }
        Bytecode::Branch(_) => "branch".to_string(),
        Bytecode::VariantSwitch(_) if branch_offsets.contains(&target_offset) => {
            "variant".to_string()
        }
        _ if target_offset == source_offset.saturating_add(1) => "fallthrough".to_string(),
        _ => "successor".to_string(),
    }
}

fn instruction_view(
    module: &CompiledModule,
    instruction: &Bytecode,
    source_map: Option<&SourceMap>,
    function_index: FunctionDefinitionIndex,
    offset: CodeOffset,
) -> MoveBytecodeInstructionView {
    MoveBytecodeInstructionView {
        offset,
        opcode: opcode_name(instruction),
        detail: format!("{instruction:?}"),
        call: instruction_call(module, instruction),
        source: source_map.and_then(|source_map| {
            source_map
                .get_code_location(function_index, offset)
                .ok()
                .map(|loc| MoveBytecodeSourceSpan {
                    start_byte: loc.start(),
                    end_byte: loc.end(),
                })
        }),
    }
}

fn instruction_call(
    module: &CompiledModule,
    instruction: &Bytecode,
) -> Option<MoveBytecodeCallView> {
    match instruction {
        Bytecode::Call(handle_index) => Some(call_view(module, *handle_index, Vec::new())),
        Bytecode::CallGeneric(instantiation_index) => {
            let instantiation = module.function_instantiation_at(*instantiation_index);
            let type_arguments = module
                .signature_at(instantiation.type_parameters)
                .0
                .iter()
                .map(|token| signature_token_label(module, token))
                .collect();

            Some(call_view(module, instantiation.handle, type_arguments))
        }
        _ => None,
    }
}

fn call_view(
    module: &CompiledModule,
    handle_index: FunctionHandleIndex,
    generic_type_arguments: Vec<String>,
) -> MoveBytecodeCallView {
    let handle = module.function_handle_at(handle_index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    let module_address = module_id.address().short_str_lossless();
    let module_name = module_id.name().to_string();
    let function_name = module.identifier_at(handle.name).to_string();
    let qualified_name = format!("{module_address}::{module_name}::{function_name}");

    MoveBytecodeCallView {
        handle_index: handle_index.0,
        module_address,
        module_name,
        function_name,
        qualified_name,
        generic_type_arguments,
    }
}
