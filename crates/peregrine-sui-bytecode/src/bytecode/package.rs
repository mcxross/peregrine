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
