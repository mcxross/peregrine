pub fn decompile_module_bytecode(
    bytecode: &[u8],
    fallback_disassembly: Option<&str>,
) -> Result<DecompiledMoveModule, String> {
    let input = MoveModuleBytecodeInput {
        name: "module".to_string(),
        bytecode: bytecode.to_vec(),
        disassembly: fallback_disassembly.map(ToString::to_string),
    };
    let mut modules = decompile_package_bytecode_modules(&[input])?;
    modules
        .pop()
        .ok_or_else(|| "Move decompiler did not return a module.".to_string())
}

pub fn decompile_package_bytecode_modules(
    modules: &[MoveModuleBytecodeInput],
) -> Result<Vec<DecompiledMoveModule>, String> {
    let mut compiled_modules = Vec::new();

    for module in modules {
        let compiled = CompiledModule::deserialize_with_defaults(&module.bytecode)
            .map_err(|error| format!("Could not deserialize module `{}`: {error}", module.name))?;
        compiled_modules.push(compiled);
    }

    let decompiled_sources = decompile_modules_with_sui_pipeline(&compiled_modules);
    let decompiler_error = decompiled_sources
        .as_ref()
        .err()
        .map(|error| error.to_string());
    let decompiled_sources = decompiled_sources.unwrap_or_default();

    modules
        .iter()
        .zip(compiled_modules.iter())
        .map(|(input, module)| {
            let name = module.self_id().name().to_string();
            let address = module.self_id().address().to_hex_literal();
            let disassembly = match input.disassembly.as_deref() {
                Some(disassembly) if !disassembly.trim().is_empty() => disassembly.to_string(),
                _ => disassemble_module(module, None)
                    .map_err(|error| format!("Could not disassemble module {name}: {error}"))?,
            };
            let source = decompiled_sources.get(&name).cloned().unwrap_or_else(|| {
                let fallback_reason = decompiler_error
                    .as_deref()
                    .unwrap_or("Sui Move decompiler did not return source for this module.");
                fallback_module_source(module, Some(fallback_reason))
            });

            Ok(DecompiledMoveModule {
                name,
                address,
                source,
                disassembly,
            })
        })
        .collect()
}

fn decompile_modules_with_sui_pipeline(
    modules: &[CompiledModule],
) -> Result<BTreeMap<String, String>, String> {
    let model_config = move_model::ModelConfig {
        allow_missing_dependencies: true,
    };
    let model = compiled_move_model::Model::from_compiled_with_config(
        model_config,
        &BTreeMap::new(),
        modules.to_vec(),
    );
    let decompiled = move_decompiler::translate::model(model)
        .map_err(|error| format!("Sui Move decompiler failed: {error}"))?;
    let move_decompiler::ast::Decompiled { model, packages } = decompiled;
    let mut sources = BTreeMap::new();

    for package in packages {
        let package_name = package
            .name
            .map(|name| name.as_str().to_owned())
            .unwrap_or_else(|| package.address.to_string());
        let Some(model_package) = model.maybe_package(&package.address) else {
            continue;
        };

        for (_, module) in package.modules {
            let module_name = module.name.as_str().to_string();
            let Some(model_module) = model_package.maybe_module(module.name) else {
                continue;
            };
            let source = move_decompiler::pretty_printer::module(
                &model,
                &package_name,
                model_module,
                &module,
            )
            .map_err(|error| {
                format!("Sui Move decompiler could not print module {module_name}: {error}")
            })?
            .render(100);

            sources.insert(module_name, source);
        }
    }

    Ok(sources)
}

fn fallback_module_source(module: &CompiledModule, decompiler_error: Option<&str>) -> String {
    let mut source = String::new();

    if let Some(error) = decompiler_error {
        source.push_str("// Sui Move decompiler could not reconstruct this module.\n");
        source.push_str(&format!(
            "// Fallback interface was generated instead: {error}\n\n"
        ));
    }

    source.push_str(&module_interface_source(module));
    source
}

pub fn decompile_module_interface_bytecode(
    bytecode: &[u8],
    fallback_disassembly: Option<&str>,
) -> Result<DecompiledMoveModule, String> {
    let module = CompiledModule::deserialize_with_defaults(bytecode)
        .map_err(|error| format!("Could not deserialize module bytecode: {error}"))?;
    let name = module.self_id().name().to_string();
    let address = module.self_id().address().to_hex_literal();
    let disassembly = match fallback_disassembly {
        Some(disassembly) if !disassembly.trim().is_empty() => disassembly.to_string(),
        _ => disassemble_module(&module, None)
            .map_err(|error| format!("Could not disassemble module {name}: {error}"))?,
    };
    let source = module_interface_source(&module);

    Ok(DecompiledMoveModule {
        name,
        address,
        source,
        disassembly,
    })
}
