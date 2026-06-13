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

