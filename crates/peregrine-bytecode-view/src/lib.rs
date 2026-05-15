use move_binary_format::file_format::{
    Bytecode, CodeOffset, CompiledModule, FunctionDefinitionIndex, FunctionHandleIndex,
    SignatureToken,
};
use move_bytecode_source_map::{
    mapping::SourceMapping, source_map::SourceMap, utils::source_map_from_file,
};
use move_disassembler::disassembler::{Disassembler, DisassemblerOptions};
use move_ir_types::location::Loc;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodePackageView {
    pub package_name: String,
    pub package_path: String,
    pub build_path: String,
    pub module_count: usize,
    pub function_count: usize,
    pub instruction_count: usize,
    pub struct_count: usize,
    pub constant_count: usize,
    pub dependency_count: usize,
    pub source_map_count: usize,
    pub modules: Vec<MoveBytecodeModuleView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeModuleView {
    pub name: String,
    pub address: String,
    pub package_name: String,
    pub is_dependency: bool,
    pub bytecode_path: String,
    pub source_map_path: Option<String>,
    pub source_path: Option<String>,
    pub byte_size: u64,
    pub version: u32,
    pub function_count: usize,
    pub instruction_count: usize,
    pub struct_count: usize,
    pub constant_count: usize,
    pub import_count: usize,
    pub friend_count: usize,
    pub functions: Vec<MoveBytecodeFunctionView>,
    pub imports: Vec<String>,
    pub disassembly: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeFunctionView {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub parameters: Vec<String>,
    pub returns: Vec<String>,
    pub type_parameter_count: usize,
    pub instruction_count: usize,
    pub local_count: usize,
    pub return_count: usize,
    pub acquires: Vec<String>,
    pub instructions: Vec<MoveBytecodeInstructionView>,
    pub control_flow: MoveBytecodeControlFlowView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeInstructionView {
    pub offset: u16,
    pub opcode: String,
    pub detail: String,
    pub call: Option<MoveBytecodeCallView>,
    pub source: Option<MoveBytecodeSourceSpan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeCallView {
    pub handle_index: u16,
    pub module_address: String,
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub generic_type_arguments: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeControlFlowView {
    pub blocks: Vec<MoveBytecodeBasicBlockView>,
    pub edges: Vec<MoveBytecodeControlFlowEdgeView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeBasicBlockView {
    pub id: String,
    pub label: String,
    pub start_offset: CodeOffset,
    pub end_offset: CodeOffset,
    pub instruction_offsets: Vec<CodeOffset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeControlFlowEdgeView {
    pub source: String,
    pub target: String,
    pub source_offset: CodeOffset,
    pub target_offset: CodeOffset,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeSourceSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

pub fn load_package_bytecode(
    package_root: impl AsRef<Path>,
    package_name: &str,
) -> Result<MoveBytecodePackageView, String> {
    let package_root = package_root.as_ref();
    let build_root = resolve_build_root(package_root, package_name)?;
    let modules_root = build_root.join("bytecode_modules");

    if !modules_root.is_dir() {
        return Err(format!(
            "No compiled bytecode found at {}. Run `sui move build` first.",
            modules_root.display()
        ));
    }

    let mut module_paths = collect_module_paths(&modules_root)?;
    module_paths.sort();

    let mut modules = Vec::new();
    let mut diagnostics = Vec::new();

    for bytecode_path in module_paths {
        match load_module_view(&build_root, package_name, &bytecode_path) {
            Ok(module) => modules.push(module),
            Err(error) => diagnostics.push(format!("{}: {error}", bytecode_path.display())),
        }
    }

    if modules.is_empty() {
        let detail = if diagnostics.is_empty() {
            "No `.mv` modules were found.".to_string()
        } else {
            diagnostics.join("\n")
        };
        return Err(format!("Could not load package bytecode. {detail}"));
    }

    let function_count = modules.iter().map(|module| module.function_count).sum();
    let instruction_count = modules.iter().map(|module| module.instruction_count).sum();
    let struct_count = modules.iter().map(|module| module.struct_count).sum();
    let constant_count = modules.iter().map(|module| module.constant_count).sum();
    let dependency_count = modules.iter().map(|module| module.import_count).sum();
    let source_map_count = modules
        .iter()
        .filter(|module| module.source_map_path.is_some())
        .count();

    Ok(MoveBytecodePackageView {
        package_name: package_name.to_string(),
        package_path: package_root.to_string_lossy().into_owned(),
        build_path: build_root.to_string_lossy().into_owned(),
        module_count: modules.len(),
        function_count,
        instruction_count,
        struct_count,
        constant_count,
        dependency_count,
        source_map_count,
        modules,
    })
}

fn resolve_build_root(package_root: &Path, package_name: &str) -> Result<PathBuf, String> {
    let build_root = package_root.join("build");
    let preferred = build_root.join(package_name);

    if preferred.join("bytecode_modules").is_dir() {
        return Ok(preferred);
    }

    let mut candidates = fs::read_dir(&build_root)
        .map_err(|error| format!("Could not read {}: {error}", build_root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("bytecode_modules").is_dir())
        .collect::<Vec<_>>();
    candidates.sort();

    match candidates.as_slice() {
        [candidate] => Ok(candidate.clone()),
        [] => Err(format!(
            "No Move build output found under {}. Run `sui move build` first.",
            build_root.display()
        )),
        _ => Err(format!(
            "Multiple build outputs found under {}; expected one matching package `{package_name}`.",
            build_root.display()
        )),
    }
}

fn collect_module_paths(modules_root: &Path) -> Result<Vec<PathBuf>, String> {
    WalkDir::new(modules_root)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry)
                if entry.file_type().is_file()
                    && entry.path().extension().and_then(|ext| ext.to_str()) == Some("mv") =>
            {
                Some(Ok(entry.path().to_path_buf()))
            }
            Ok(_) => None,
            Err(error) => Some(Err(format!("Could not read bytecode module path: {error}"))),
        })
        .collect()
}

fn load_module_view(
    build_root: &Path,
    root_package_name: &str,
    bytecode_path: &Path,
) -> Result<MoveBytecodeModuleView, String> {
    let bytecode = fs::read(bytecode_path)
        .map_err(|error| format!("Could not read {}: {error}", bytecode_path.display()))?;
    let byte_size = bytecode.len() as u64;
    let module = CompiledModule::deserialize_with_defaults(&bytecode)
        .map_err(|error| format!("Could not deserialize module bytecode: {error}"))?;
    let relative_module_path = bytecode_path
        .strip_prefix(build_root.join("bytecode_modules"))
        .unwrap_or(bytecode_path);
    let (module_package_name, is_dependency) =
        module_package_origin(relative_module_path, root_package_name);
    let source_map_path = build_root
        .join("debug_info")
        .join(relative_module_path)
        .with_extension("mvd");
    let source_path = build_root
        .join("sources")
        .join(relative_module_path)
        .with_extension("move");
    let source_map = if source_map_path.is_file() {
        source_map_from_file(&source_map_path).ok()
    } else {
        None
    };
    let functions = module_functions(&module, source_map.as_ref());
    let instruction_count = functions
        .iter()
        .map(|function| function.instruction_count)
        .sum();
    let imports = module_imports(&module);

    Ok(MoveBytecodeModuleView {
        name: module.self_id().name().to_string(),
        address: module.self_id().address().short_str_lossless(),
        package_name: module_package_name,
        is_dependency,
        bytecode_path: bytecode_path.to_string_lossy().into_owned(),
        source_map_path: source_map_path
            .is_file()
            .then(|| source_map_path.to_string_lossy().into_owned()),
        source_path: source_path
            .is_file()
            .then(|| source_path.to_string_lossy().into_owned()),
        byte_size,
        version: module.version(),
        function_count: module.function_defs().len(),
        instruction_count,
        struct_count: module.struct_defs().len(),
        constant_count: module.constant_pool().len(),
        import_count: imports.len(),
        friend_count: module.friend_decls().len(),
        functions,
        imports,
        disassembly: disassemble_module(&module, source_map).unwrap_or_else(|error| {
            format!("Could not disassemble {}: {error}", bytecode_path.display())
        }),
    })
}

fn module_package_origin(relative_module_path: &Path, root_package_name: &str) -> (String, bool) {
    let components = relative_module_path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();

    match components.as_slice() {
        ["dependencies", dependency_name, ..] => ((*dependency_name).to_string(), true),
        _ => (root_package_name.to_string(), false),
    }
}

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

fn opcode_name(instruction: &Bytecode) -> String {
    let detail = format!("{instruction:?}");
    let end = detail.find(['(', ' ']).unwrap_or(detail.len());
    detail[..end].to_string()
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
            let (index, type_arguments) = &**instantiation;
            let arguments = type_arguments
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
        module.identifier_at(handle.name),
    )
}

fn module_imports(module: &CompiledModule) -> Vec<String> {
    let self_id = module.self_id();
    let mut imports = BTreeMap::new();

    for handle in module.module_handles() {
        let id = module.module_id_for_handle(handle);

        if id == self_id {
            continue;
        }

        imports.insert(
            format!("{}::{}", id.address().short_str_lossless(), id.name()),
            (),
        );
    }

    imports.into_keys().collect()
}

fn disassemble_module(
    module: &CompiledModule,
    source_map: Option<SourceMap>,
) -> Result<String, String> {
    let mut options = DisassemblerOptions::new();
    options.print_code = true;
    options.print_basic_blocks = true;

    let disassembler = if let Some(source_map) = source_map {
        Disassembler::new(SourceMapping::new(source_map, module), options)
    } else {
        Disassembler::new(
            SourceMapping::new_without_source_map(module, Loc::invalid())
                .map_err(|error| error.to_string())?,
            options,
        )
    };

    disassembler
        .disassemble()
        .map_err(|error| error.to_string())
}
