use move_binary_format::file_format::CompiledModule;
use peregrine_bytecode::{
    decompile_package_bytecode_modules, DecompiledMoveModule, MoveModuleBytecodeInput,
};
use peregrine_package_resolution::sui::{
    fetch_move_package_from_graphql, normalize_sui_package_id, FetchedMovePackage,
};
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const RAW_DIRECTORY: &str = "raw";
const BUILDABLE_DIRECTORY: &str = "buildable";
const IMPORTED_PACKAGES_DIRECTORY: &str = "imported-packages";
const PROJECT_METADATA_DIRECTORY: &str = ".peregrine";
const IMPORT_ENGINE_METADATA_FILE: &str = "import-engine.json";

#[derive(Clone, Debug)]
pub struct ImportEngine {
    config: ImportEngineConfig,
}

#[derive(Clone, Debug)]
pub struct ImportEngineConfig {
    pub max_dependency_depth: usize,
    pub max_dependency_packages: usize,
    pub build_verification: BuildVerification,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildVerification {
    Disabled,
    SystemSui {
        executable: PathBuf,
        default_move_flavor: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct BuildableImportRequest {
    pub network_id: String,
    pub graph_ql_url: String,
    pub package_id: String,
    pub import_root: PathBuf,
    pub generate_buildable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildableImportArtifact {
    pub raw_root: PathBuf,
    pub buildable_root: PathBuf,
    pub project_root: PathBuf,
    pub root_package_id: String,
    pub root_package_name: String,
    pub dependencies: Vec<BuildableDependency>,
    pub diagnostics: Vec<EngineDiagnostic>,
    pub build_result: Option<BuildResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildableDependency {
    pub package_id: String,
    pub package_name: String,
    pub depth: usize,
    pub local_path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineDiagnostic {
    pub severity: EngineDiagnosticSeverity,
    pub stage: String,
    pub package_id: Option<String>,
    pub module: Option<String>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    pub command: String,
    pub success: bool,
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone)]
struct ResolvedPackage {
    package: FetchedMovePackage,
    depth: usize,
}

#[derive(Clone)]
struct MaterializedMoveModule {
    name: String,
    bytecode: Vec<u8>,
    decompiled: DecompiledMoveModule,
}

#[derive(Clone, Debug)]
struct AddressAlias {
    address: String,
    hex: String,
    alias: String,
    kind: AliasKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AliasKind {
    Root,
    Dependency,
    Framework,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportEngineMetadata<'a> {
    package_id: &'a str,
    network_id: &'a str,
    graph_ql_url: &'a str,
    version: u64,
    digest: &'a str,
    module_count: usize,
    dependency_count: usize,
    imported_at: u64,
}

impl Default for ImportEngine {
    fn default() -> Self {
        Self {
            config: ImportEngineConfig::default(),
        }
    }
}

impl ImportEngine {
    pub fn new(config: ImportEngineConfig) -> Self {
        Self { config }
    }

    pub async fn import_buildable_package(
        &self,
        request: BuildableImportRequest,
    ) -> Result<BuildableImportArtifact, String> {
        let package_id = normalize_sui_package_id(&request.package_id)?;
        let root_package =
            fetch_move_package_from_graphql(&request.graph_ql_url, &package_id).await?;
        let mut packages = BTreeMap::from([(
            normalize_sui_package_id(&root_package.address)?,
            ResolvedPackage {
                package: root_package,
                depth: 0,
            },
        )]);
        let mut queue = VecDeque::from([(package_id.clone(), 0usize)]);

        while request.generate_buildable {
            let Some((current_package_id, current_depth)) = queue.pop_front() else {
                break;
            };
            let Some(current_package) = packages.get(&current_package_id).cloned() else {
                continue;
            };
            let dependencies = package_dependency_addresses(&current_package.package)?;

            for dependency_id in dependencies {
                if is_framework_address(&dependency_id) || packages.contains_key(&dependency_id) {
                    continue;
                }

                if current_depth + 1 > self.config.max_dependency_depth {
                    return Err(format!(
                        "Package {current_package_id} references {dependency_id}, which exceeds the configured dependency depth of {}.",
                        self.config.max_dependency_depth
                    ));
                }

                let dependency_count = packages
                    .values()
                    .filter(|package| package.depth > 0)
                    .count();
                if dependency_count >= self.config.max_dependency_packages {
                    return Err(format!(
                        "Package {current_package_id} references {dependency_id}, but the configured dependency package limit of {} has been reached.",
                        self.config.max_dependency_packages
                    ));
                }

                let fetched =
                    fetch_move_package_from_graphql(&request.graph_ql_url, &dependency_id).await?;
                let fetched_id = normalize_sui_package_id(&fetched.address)?;
                packages.insert(
                    fetched_id.clone(),
                    ResolvedPackage {
                        package: fetched,
                        depth: current_depth + 1,
                    },
                );
                queue.push_back((fetched_id, current_depth + 1));
            }
        }

        self.materialize_resolved_packages(request, package_id, packages)
    }

    pub fn migrate_fetched_packages(
        &self,
        request: BuildableImportRequest,
        packages: Vec<FetchedMovePackage>,
    ) -> Result<BuildableImportArtifact, String> {
        let package_id = normalize_sui_package_id(&request.package_id)?;
        let mut resolved_packages = BTreeMap::new();

        for package in packages {
            let address = normalize_sui_package_id(&package.address)?;
            let depth = if address == package_id { 0 } else { 1 };
            resolved_packages.insert(address, ResolvedPackage { package, depth });
        }

        if !resolved_packages.contains_key(&package_id) {
            return Err(format!(
                "Fetched package set does not contain requested root package {package_id}."
            ));
        }

        self.materialize_resolved_packages(request, package_id, resolved_packages)
    }

    fn materialize_resolved_packages(
        &self,
        request: BuildableImportRequest,
        root_package_id: String,
        packages: BTreeMap<String, ResolvedPackage>,
    ) -> Result<BuildableImportArtifact, String> {
        let root_package = packages
            .get(&root_package_id)
            .ok_or_else(|| format!("Resolved package set does not contain {root_package_id}."))?;
        let raw_root = if request.generate_buildable {
            request.import_root.join(RAW_DIRECTORY)
        } else {
            request.import_root.clone()
        };
        let buildable_root = request.import_root.join(BUILDABLE_DIRECTORY);
        let mut alias_package_ids = packages.keys().cloned().collect::<BTreeSet<_>>();
        if !request.generate_buildable {
            alias_package_ids.extend(package_dependency_addresses(&root_package.package)?);
        }
        let aliases = address_aliases(&root_package_id, alias_package_ids.iter())?;
        let root_alias = alias_for_address(&aliases, &root_package_id)
            .ok_or_else(|| format!("Could not assign an address alias for {root_package_id}."))?;
        let mut diagnostics = Vec::new();
        let mut dependencies = Vec::new();

        if request.generate_buildable {
            replace_directory(&raw_root)?;
            replace_directory(&buildable_root)?;
        } else if buildable_root.exists() {
            fs::remove_dir_all(&buildable_root).map_err(|error| {
                format!(
                    "Could not remove stale buildable artifact at {}: {error}",
                    buildable_root.display()
                )
            })?;
        }
        if !request.generate_buildable {
            let stale_raw_root = request.import_root.join(RAW_DIRECTORY);
            if stale_raw_root.exists() {
                fs::remove_dir_all(&stale_raw_root).map_err(|error| {
                    format!(
                        "Could not remove stale raw artifact at {}: {error}",
                        stale_raw_root.display()
                    )
                })?;
            }
        }
        if !request.generate_buildable {
            fs::create_dir_all(&raw_root)
                .map_err(|error| format!("Could not create {}: {error}", raw_root.display()))?;
            let root_package_path = raw_root.join(&root_alias.alias);
            if root_package_path.exists() {
                fs::remove_dir_all(&root_package_path).map_err(|error| {
                    format!(
                        "Could not replace raw package at {}: {error}",
                        root_package_path.display()
                    )
                })?;
            }
        }
        fs::create_dir_all(request.import_root.join(PROJECT_METADATA_DIRECTORY)).map_err(
            |error| {
                format!(
                    "Could not create {}: {error}",
                    request
                        .import_root
                        .join(PROJECT_METADATA_DIRECTORY)
                        .display()
                )
            },
        )?;

        write_import_metadata(
            &request,
            root_package,
            packages
                .values()
                .filter(|package| package.depth > 0)
                .count(),
        )?;

        for (package_id, resolved) in &packages {
            let alias = alias_for_address(&aliases, package_id)
                .ok_or_else(|| format!("Could not assign an address alias for {package_id}."))?;
            let package_root = if package_id == &root_package_id {
                buildable_root.clone()
            } else {
                buildable_root.join("deps").join(&alias.alias)
            };
            let direct_dependencies = package_dependency_addresses(&resolved.package)?
                .into_iter()
                .filter(|dependency_id| dependency_id != package_id)
                .collect::<BTreeSet<_>>();

            write_raw_package(&raw_root, alias, &aliases, &resolved.package)?;

            if request.generate_buildable {
                write_buildable_package(
                    &package_root,
                    &resolved.package,
                    alias,
                    &aliases,
                    &direct_dependencies,
                    &mut diagnostics,
                )?;

                if package_id != &root_package_id {
                    dependencies.push(BuildableDependency {
                        package_id: package_id.clone(),
                        package_name: alias.alias.clone(),
                        depth: resolved.depth,
                        local_path: package_root,
                    });
                }
            }
        }

        dependencies.sort_by(|left, right| left.package_id.cmp(&right.package_id));

        let build_result = if request.generate_buildable {
            match self.config.build_verification {
                BuildVerification::Disabled => None,
                BuildVerification::SystemSui {
                    ref executable,
                    ref default_move_flavor,
                } => {
                    let result = run_system_move_build(
                        executable,
                        &buildable_root,
                        default_move_flavor.as_deref(),
                    );
                    if !result.success {
                        diagnostics.push(EngineDiagnostic {
                            severity: EngineDiagnosticSeverity::Error,
                            stage: "build".to_string(),
                            package_id: Some(root_package_id.clone()),
                            module: None,
                            message: result.stderr.clone(),
                        });
                    }
                    Some(result)
                }
            }
        } else {
            None
        };
        let project_root = if request.generate_buildable {
            buildable_root.clone()
        } else {
            raw_root.join(&root_alias.alias)
        };

        Ok(BuildableImportArtifact {
            raw_root,
            buildable_root,
            project_root,
            root_package_id,
            root_package_name: root_alias.alias.clone(),
            dependencies,
            diagnostics,
            build_result,
        })
    }
}

impl Default for ImportEngineConfig {
    fn default() -> Self {
        Self {
            max_dependency_depth: 3,
            max_dependency_packages: 64,
            build_verification: BuildVerification::SystemSui {
                executable: PathBuf::from("sui"),
                default_move_flavor: None,
            },
        }
    }
}

pub fn default_import_root(
    app_data_dir: impl AsRef<Path>,
    network_id: &str,
    package_id: &str,
) -> Result<PathBuf, String> {
    let package_id = normalize_sui_package_id(package_id)?;
    Ok(app_data_dir
        .as_ref()
        .join(IMPORTED_PACKAGES_DIRECTORY)
        .join(sanitize_path_component(network_id))
        .join(sanitize_path_component(package_id.trim_start_matches("0x"))))
}

fn package_dependency_addresses(package: &FetchedMovePackage) -> Result<BTreeSet<String>, String> {
    let self_address = normalize_sui_package_id(&package.address)?;
    let mut dependencies = BTreeSet::new();

    for module in &package.modules {
        let compiled = CompiledModule::deserialize_with_defaults(&module.bytecode)
            .map_err(|error| format!("Could not deserialize module `{}`: {error}", module.name))?;

        for handle in compiled.module_handles() {
            let id = compiled.module_id_for_handle(handle);
            let address = normalize_sui_package_id(&id.address().to_hex_literal())?;

            if address != self_address {
                dependencies.insert(address);
            }
        }
    }

    Ok(dependencies)
}

fn address_aliases<'a>(
    root_package_id: &str,
    package_ids: impl Iterator<Item = &'a String>,
) -> Result<Vec<AddressAlias>, String> {
    let mut aliases = vec![
        AddressAlias {
            address: "0x1".to_string(),
            hex: "1".to_string(),
            alias: "std".to_string(),
            kind: AliasKind::Framework,
        },
        AddressAlias {
            address: "0x2".to_string(),
            hex: "2".to_string(),
            alias: "sui".to_string(),
            kind: AliasKind::Framework,
        },
        AddressAlias {
            address: "0x3".to_string(),
            hex: "3".to_string(),
            alias: "sui_system".to_string(),
            kind: AliasKind::Framework,
        },
        AddressAlias {
            address: "0xb".to_string(),
            hex: "b".to_string(),
            alias: "bridge".to_string(),
            kind: AliasKind::Framework,
        },
    ];
    let mut used = BTreeSet::from([
        "bridge".to_string(),
        "std".to_string(),
        "sui".to_string(),
        "sui_system".to_string(),
    ]);
    let root_package_id = normalize_sui_package_id(root_package_id)?;

    for package_id in package_ids {
        let address = normalize_sui_package_id(package_id)?;

        if is_framework_address(&address) {
            continue;
        }

        let kind = if address == root_package_id {
            AliasKind::Root
        } else {
            AliasKind::Dependency
        };
        let prefix = if kind == AliasKind::Root {
            "pkg"
        } else {
            "dep"
        };
        let hex = normalized_hex(&address)?;
        let suffix = hex
            .chars()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        let base = format!("{prefix}_{suffix}");
        let alias = unique_alias(base, &mut used);

        aliases.push(AddressAlias {
            address,
            hex,
            alias,
            kind,
        });
    }

    aliases.sort_by(|left, right| {
        right
            .hex
            .len()
            .cmp(&left.hex.len())
            .then_with(|| left.alias.cmp(&right.alias))
    });

    Ok(aliases)
}

fn alias_for_address<'a>(aliases: &'a [AddressAlias], address: &str) -> Option<&'a AddressAlias> {
    let address = normalize_sui_package_id(address).ok()?;
    aliases.iter().find(|alias| alias.address == address)
}

fn unique_alias(base: String, used: &mut BTreeSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }

    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded alias suffix search should return")
}

fn is_framework_address(address: &str) -> bool {
    matches!(
        normalize_sui_package_id(address).as_deref(),
        Ok("0x1" | "0x2" | "0x3" | "0xb")
    )
}

fn write_import_metadata(
    request: &BuildableImportRequest,
    root_package: &ResolvedPackage,
    dependency_count: usize,
) -> Result<(), String> {
    let metadata = ImportEngineMetadata {
        package_id: &root_package.package.address,
        network_id: &request.network_id,
        graph_ql_url: &request.graph_ql_url,
        version: root_package.package.version,
        digest: &root_package.package.digest,
        module_count: root_package.package.modules.len(),
        dependency_count,
        imported_at: now_unix_seconds(),
    };
    let metadata = serde_json::to_string_pretty(&metadata)
        .map_err(|error| format!("Could not serialize import engine metadata: {error}"))?;
    let metadata_path = request
        .import_root
        .join(PROJECT_METADATA_DIRECTORY)
        .join(IMPORT_ENGINE_METADATA_FILE);

    fs::write(&metadata_path, metadata)
        .map_err(|error| format!("Could not write {}: {error}", metadata_path.display()))
}

fn write_raw_package(
    raw_root: &Path,
    alias: &AddressAlias,
    aliases: &[AddressAlias],
    package: &FetchedMovePackage,
) -> Result<(), String> {
    let package_root = raw_root.join(&alias.alias);
    let sources_dir = package_root.join("sources");
    let bytecode_dir = package_root.join("bytecode_modules");
    let decompiled_dir = package_root.join("decompiled");
    let standard_build_root = package_root.join("build").join(&alias.alias);
    let standard_build_sources_dir = standard_build_root.join("sources");
    let standard_build_bytecode_dir = standard_build_root.join("bytecode_modules");
    let materialized = decompile_fetched_modules(package, &mut Vec::new())?;

    fs::create_dir_all(&sources_dir)
        .map_err(|error| format!("Could not create {}: {error}", sources_dir.display()))?;
    fs::create_dir_all(&bytecode_dir)
        .map_err(|error| format!("Could not create {}: {error}", bytecode_dir.display()))?;
    fs::create_dir_all(&decompiled_dir)
        .map_err(|error| format!("Could not create {}: {error}", decompiled_dir.display()))?;
    fs::create_dir_all(&standard_build_sources_dir).map_err(|error| {
        format!(
            "Could not create {}: {error}",
            standard_build_sources_dir.display()
        )
    })?;
    fs::create_dir_all(&standard_build_bytecode_dir).map_err(|error| {
        format!(
            "Could not create {}: {error}",
            standard_build_bytecode_dir.display()
        )
    })?;

    let manifest = raw_package_manifest_source(alias, aliases);
    fs::write(package_root.join("Move.toml"), manifest)
        .map_err(|error| format!("Could not write raw package manifest: {error}"))?;

    for module in materialized {
        let file_stem = module_file_stem(&module.name, &module.decompiled.name);
        let source = rewrite_source_addresses(&module.decompiled.source, aliases)?;
        fs::write(
            bytecode_dir.join(format!("{file_stem}.mv")),
            &module.bytecode,
        )
        .map_err(|error| format!("Could not write raw bytecode module: {error}"))?;
        fs::write(
            standard_build_bytecode_dir.join(format!("{file_stem}.mv")),
            &module.bytecode,
        )
        .map_err(|error| format!("Could not write standard build bytecode module: {error}"))?;
        fs::write(decompiled_dir.join(format!("{file_stem}.move")), &source)
            .map_err(|error| format!("Could not write raw decompiled source: {error}"))?;
        fs::write(sources_dir.join(format!("{file_stem}.move")), &source)
            .map_err(|error| format!("Could not write raw source module: {error}"))?;
        fs::write(
            standard_build_sources_dir.join(format!("{file_stem}.move")),
            &source,
        )
        .map_err(|error| format!("Could not write standard build source module: {error}"))?;
        fs::write(
            decompiled_dir.join(format!("{file_stem}.moveasm")),
            module.decompiled.disassembly,
        )
        .map_err(|error| format!("Could not write raw disassembly: {error}"))?;
    }

    Ok(())
}

fn raw_package_manifest_source(package_alias: &AddressAlias, aliases: &[AddressAlias]) -> String {
    let mut manifest = format!(
        r#"[package]
name = "{}"
edition = "2024"
published-at = "{}"
"#,
        package_alias.alias, package_alias.address
    );

    manifest.push_str("\n[addresses]\n");

    let mut address_entries = aliases
        .iter()
        .map(|alias| (alias.alias.clone(), alias.address.clone()))
        .collect::<Vec<_>>();
    address_entries.sort_by(|left, right| left.0.cmp(&right.0));

    for (alias, address) in address_entries {
        manifest.push_str(&format!("{alias} = \"{address}\"\n"));
    }

    manifest
}

fn write_buildable_package(
    package_root: &Path,
    package: &FetchedMovePackage,
    package_alias: &AddressAlias,
    aliases: &[AddressAlias],
    direct_dependencies: &BTreeSet<String>,
    diagnostics: &mut Vec<EngineDiagnostic>,
) -> Result<(), String> {
    let sources_dir = package_root.join("sources");
    fs::create_dir_all(&sources_dir)
        .map_err(|error| format!("Could not create {}: {error}", sources_dir.display()))?;

    let materialized = decompile_fetched_modules(package, diagnostics)?;
    let mut sources = BTreeMap::new();
    let mut package_has_friends = false;

    for module in materialized {
        let file_stem = module_file_stem(&module.name, &module.decompiled.name);
        let friend_paths = module_friend_paths(&module.bytecode)?;
        package_has_friends |= !friend_paths.is_empty();
        let friend_paths = friend_paths
            .into_iter()
            .map(|friend| rewrite_source_addresses(&friend, aliases))
            .collect::<Result<Vec<_>, _>>()?;
        let source = rewrite_source_addresses(&module.decompiled.source, aliases)?;
        let source = add_friend_declarations(&source, &friend_paths);
        sources.insert(file_stem, source);
    }

    migrate_shared_constants(&mut sources, &package_alias.alias)?;

    if package_has_friends {
        for source in sources.values_mut() {
            *source = convert_label_module_to_legacy_block(source);
        }
    }

    for (file_stem, source) in sources {
        let source_path = sources_dir.join(format!("{file_stem}.move"));
        fs::write(&source_path, source)
            .map_err(|error| format!("Could not write {}: {error}", source_path.display()))?;
    }

    let edition = if package_has_friends {
        "legacy"
    } else {
        "2024"
    };
    let manifest = package_manifest_source(package_alias, aliases, direct_dependencies, edition)?;
    let manifest_path = package_root.join("Move.toml");
    fs::write(&manifest_path, manifest)
        .map_err(|error| format!("Could not write {}: {error}", manifest_path.display()))
}

fn decompile_fetched_modules(
    package: &FetchedMovePackage,
    diagnostics: &mut Vec<EngineDiagnostic>,
) -> Result<Vec<MaterializedMoveModule>, String> {
    let decompile_inputs = package
        .modules
        .iter()
        .map(|module| MoveModuleBytecodeInput {
            name: module.name.clone(),
            bytecode: module.bytecode.clone(),
            disassembly: module.disassembly.clone(),
        })
        .collect::<Vec<_>>();
    let decompiled_modules = decompile_package_bytecode_modules(&decompile_inputs)?;

    Ok(package
        .modules
        .iter()
        .cloned()
        .zip(decompiled_modules)
        .map(|(module, decompiled)| {
            let is_fallback = decompiled
                .source
                .contains("Fallback interface was generated");
            diagnostics.push(EngineDiagnostic {
                severity: if is_fallback {
                    EngineDiagnosticSeverity::Warning
                } else {
                    EngineDiagnosticSeverity::Info
                },
                stage: "decompile".to_string(),
                package_id: Some(package.address.clone()),
                module: Some(decompiled.name.clone()),
                message: if is_fallback {
                    "Sui decompiler failed; generated interface fallback for this module."
                        .to_string()
                } else {
                    "Sui decompiler reconstructed source for this module.".to_string()
                },
            });

            MaterializedMoveModule {
                name: module.name,
                bytecode: module.bytecode,
                decompiled,
            }
        })
        .collect())
}

fn package_manifest_source(
    package_alias: &AddressAlias,
    aliases: &[AddressAlias],
    direct_dependencies: &BTreeSet<String>,
    edition: &str,
) -> Result<String, String> {
    let mut manifest = format!(
        r#"[package]
name = "{}"
edition = "{edition}"
published-at = "{}"
"#,
        package_alias.alias, package_alias.address
    );
    let dependency_entries =
        dependency_manifest_entries(package_alias, aliases, direct_dependencies)?;

    if !dependency_entries.is_empty() {
        manifest.push_str("\n[dependencies]\n");
        for (name, value) in dependency_entries {
            manifest.push_str(&format!("{name} = {{ local = \"{value}\" }}\n"));
        }
    }

    manifest.push_str("\n[addresses]\n");

    let mut address_entries = aliases
        .iter()
        .map(|alias| (alias.alias.clone(), alias.address.clone()))
        .collect::<Vec<_>>();
    address_entries.sort_by(|left, right| left.0.cmp(&right.0));

    for (alias, address) in address_entries {
        manifest.push_str(&format!("{alias} = \"{address}\"\n"));
    }

    Ok(manifest)
}

fn dependency_manifest_entries(
    package_alias: &AddressAlias,
    aliases: &[AddressAlias],
    direct_dependencies: &BTreeSet<String>,
) -> Result<Vec<(String, String)>, String> {
    let mut entries = Vec::new();

    for dependency_id in direct_dependencies {
        if is_framework_address(dependency_id) {
            continue;
        }

        let dependency_alias = alias_for_address(aliases, dependency_id)
            .ok_or_else(|| format!("No package alias found for dependency {dependency_id}."))?;
        let path = if package_alias.kind == AliasKind::Root {
            format!("deps/{}", dependency_alias.alias)
        } else {
            format!("../{}", dependency_alias.alias)
        };
        entries.push((dependency_alias.alias.clone(), path));
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

fn rewrite_source_addresses(source: &str, aliases: &[AddressAlias]) -> Result<String, String> {
    let mut rewritten = source.to_string();

    for alias in aliases {
        let hex = regex::escape(&alias.hex);
        let path_with_prefix = RegexBuilder::new(&format!(r"(?i)\b0x0*{hex}(::)"))
            .build()
            .map_err(|error| format!("Could not build address rewrite regex: {error}"))?;
        let path_replacement = format!("{}${{1}}", alias.alias);
        rewritten = path_with_prefix
            .replace_all(&rewritten, path_replacement.as_str())
            .into_owned();

        let address_literal = RegexBuilder::new(&format!(r"(?i)@0x0*{hex}\b"))
            .build()
            .map_err(|error| format!("Could not build address literal rewrite regex: {error}"))?;
        rewritten = address_literal
            .replace_all(&rewritten, format!("@{}", alias.alias))
            .into_owned();

        let bare_path = RegexBuilder::new(&format!(r"(?i)(^|[^A-Za-z0-9_])0*{hex}(::)"))
            .multi_line(true)
            .build()
            .map_err(|error| format!("Could not build bare address rewrite regex: {error}"))?;
        let bare_replacement = format!("${{1}}{}${{2}}", alias.alias);
        rewritten = bare_path
            .replace_all(&rewritten, bare_replacement.as_str())
            .into_owned();
    }

    Ok(rewritten)
}

fn module_friend_paths(bytecode: &[u8]) -> Result<Vec<String>, String> {
    let module = CompiledModule::deserialize_with_defaults(bytecode)
        .map_err(|error| format!("Could not deserialize module bytecode for friends: {error}"))?;

    Ok(module
        .immediate_friends()
        .into_iter()
        .map(|friend| format!("{}::{}", friend.address().to_hex_literal(), friend.name()))
        .collect())
}

fn add_friend_declarations(source: &str, friend_paths: &[String]) -> String {
    let missing_friends = friend_paths
        .iter()
        .filter(|friend| !source.contains(&format!("friend {friend};")))
        .collect::<Vec<_>>();

    if missing_friends.is_empty() {
        return source.to_string();
    }

    let declarations = missing_friends
        .into_iter()
        .map(|friend| format!("    friend {friend};"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut rewritten = String::with_capacity(source.len() + declarations.len() + 2);
    let mut inserted = false;

    for line in source.lines() {
        rewritten.push_str(line);
        rewritten.push('\n');

        if !inserted
            && line.trim_start().starts_with("module ")
            && (line.trim_end().ends_with(';') || line.contains('{'))
        {
            rewritten.push_str(&declarations);
            rewritten.push('\n');
            inserted = true;
        }
    }

    if inserted {
        rewritten
    } else {
        format!("{declarations}\n{source}")
    }
}

fn convert_label_module_to_legacy_block(source: &str) -> String {
    let mut rewritten = String::with_capacity(source.len() + 4);
    let mut converted = false;

    for line in source.lines() {
        if !converted && line.trim_start().starts_with("module ") && line.trim_end().ends_with(';')
        {
            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];
            let module = line.trim_end().trim_end_matches(';');
            rewritten.push_str(indent);
            rewritten.push_str(module.trim_start());
            rewritten.push_str(" {\n");
            converted = true;
        } else {
            rewritten.push_str(line);
            rewritten.push('\n');
        }
    }

    if converted {
        rewritten.push_str("}\n");
        rewritten
    } else {
        source.to_string()
    }
}

fn migrate_shared_constants(
    sources: &mut BTreeMap<String, String>,
    package_alias: &str,
) -> Result<(), String> {
    let constants = duplicated_literal_constants(sources)?;

    if constants.is_empty() {
        return Ok(());
    }

    let mut generated =
        String::from("\n// Shared constants lifted from repeated decompiler literals.\n");
    for constant in constants {
        generated.push_str(&format!(
            "public fun {}(): {} {{\n    {}\n}}\n\n",
            constant.function_name, constant.type_name, constant.value
        ));
    }

    sources
        .entry("constants".to_string())
        .and_modify(|source| {
            source.push_str(&generated);
        })
        .or_insert_with(|| format!("module {package_alias}::constants;\n{generated}"));

    Ok(())
}

struct SharedConstant {
    function_name: String,
    type_name: String,
    value: String,
}

fn duplicated_literal_constants(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<SharedConstant>, String> {
    let const_regex =
        Regex::new(r"(?m)^\s*const\s+C[0-9A-Za-z_]*\s*:\s*([A-Za-z0-9_:<>]+)\s*=\s*([^;\n]+);\s*$")
            .map_err(|error| format!("Could not build constant scanner regex: {error}"))?;
    let mut occurrences = BTreeMap::<(String, String), usize>::new();

    for source in sources.values() {
        for capture in const_regex.captures_iter(source) {
            let type_name = capture[1].trim().to_string();
            let value = capture[2].trim().to_string();

            if is_shareable_constant_value(&type_name, &value) {
                *occurrences.entry((type_name, value)).or_default() += 1;
            }
        }
    }

    let mut constants = occurrences
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|((type_name, value), _)| {
            let digest = Sha256::digest(format!("{type_name}:{value}").as_bytes());
            let suffix = hex::encode(digest)[..10].to_string();

            SharedConstant {
                function_name: format!("shared_{suffix}"),
                type_name,
                value,
            }
        })
        .collect::<Vec<_>>();
    constants.sort_by(|left, right| left.function_name.cmp(&right.function_name));

    Ok(constants)
}

fn is_shareable_constant_value(type_name: &str, value: &str) -> bool {
    matches!(
        type_name,
        "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "u256" | "address"
    ) && value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '_' | 'x' | 'X' | '@')
    })
}

fn run_system_move_build(
    executable: &Path,
    package_root: &Path,
    default_move_flavor: Option<&str>,
) -> BuildResult {
    let mut command = format!(
        "{} move build --path {}",
        executable.display(),
        package_root.display()
    );
    if let Some(default_move_flavor) = default_move_flavor {
        command.push_str(&format!(" --default-move-flavor {default_move_flavor}"));
    }
    let move_home = package_root.join(".move-home");
    let sui_config_dir = package_root.join(".sui-config");

    if let Err(error) = fs::create_dir_all(&move_home) {
        return BuildResult {
            command,
            success: false,
            status: None,
            stdout: String::new(),
            stderr: format!("Could not create {}: {error}", move_home.display()),
        };
    }

    if let Err(error) = fs::create_dir_all(&sui_config_dir) {
        return BuildResult {
            command,
            success: false,
            status: None,
            stdout: String::new(),
            stderr: format!("Could not create {}: {error}", sui_config_dir.display()),
        };
    }

    let mut build_command = Command::new(executable);
    build_command
        .args(["move", "build", "--path"])
        .arg(package_root);
    if let Some(default_move_flavor) = default_move_flavor {
        build_command.args(["--default-move-flavor", default_move_flavor]);
    }

    match build_command
        .env("MOVE_HOME", &move_home)
        .env("SUI_CONFIG_DIR", &sui_config_dir)
        .output()
    {
        Ok(output) => BuildResult {
            command,
            success: output.status.success(),
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        Err(error) => BuildResult {
            command,
            success: false,
            status: None,
            stdout: String::new(),
            stderr: format!("Could not run Sui build command: {error}"),
        },
    }
}

fn module_file_stem(graphql_name: &str, decompiled_name: &str) -> String {
    let stem = sanitize_path_component(decompiled_name);

    if stem == "_" {
        sanitize_path_component(graphql_name)
    } else {
        stem
    }
}

fn replace_directory(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Could not replace {}: {error}", path.display()))?;
    }

    fs::create_dir_all(path)
        .map_err(|error| format!("Could not create {}: {error}", path.display()))
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('.').trim_matches('_');

    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized.to_string()
    }
}

fn normalized_hex(address: &str) -> Result<String, String> {
    let address = normalize_sui_package_id(address)?;
    Ok(address
        .trim_start_matches("0x")
        .trim_start_matches('0')
        .to_ascii_lowercase()
        .chars()
        .collect::<String>()
        .if_empty_then("0"))
}

trait EmptyStringDefault {
    fn if_empty_then(self, default: &str) -> String;
}

impl EmptyStringDefault for String {
    fn if_empty_then(self, default: &str) -> String {
        if self.is_empty() {
            default.to_string()
        } else {
            self
        }
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_package_resolution::sui::FetchedMoveModule;
    use tempfile::tempdir;

    fn fixture_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("import-engine crate should have a crates parent")
            .join("peregrine-indexer/tests/fixtures/sui")
            .join(relative)
    }

    fn fetched_fixture_package(relative: &str, package_id: &str) -> FetchedMovePackage {
        let bytecode_dir = fixture_path(relative)
            .join("build")
            .join(relative)
            .join("bytecode_modules");
        let mut modules = fs::read_dir(&bytecode_dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", bytecode_dir.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("mv"))
            .collect::<Vec<_>>();
        modules.sort();

        FetchedMovePackage {
            address: package_id.to_string(),
            version: 1,
            digest: "fixture".to_string(),
            modules: modules
                .into_iter()
                .map(|path| FetchedMoveModule {
                    name: path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("module")
                        .to_string(),
                    bytecode: fs::read(&path).expect("fixture bytecode"),
                    disassembly: None,
                })
                .collect(),
        }
    }

    fn disabled_engine() -> ImportEngine {
        ImportEngine::new(ImportEngineConfig {
            build_verification: BuildVerification::Disabled,
            ..ImportEngineConfig::default()
        })
    }

    #[test]
    fn migrates_fixture_package_with_decompiler_source_and_named_address() {
        let temp = tempdir().expect("tempdir");
        let package = fetched_fixture_package("friend_function", "0x0");
        let request = BuildableImportRequest {
            network_id: "testnet".to_string(),
            graph_ql_url: "https://graphql.testnet.sui.io/graphql".to_string(),
            package_id: "0x0".to_string(),
            import_root: temp.path().join("import"),
            generate_buildable: true,
        };

        let artifact = disabled_engine()
            .migrate_fetched_packages(request, vec![package])
            .expect("migrate fixture");

        let source = fs::read_to_string(artifact.buildable_root.join("sources/a.move"))
            .expect("migrated source");
        let manifest =
            fs::read_to_string(artifact.buildable_root.join("Move.toml")).expect("manifest");

        assert!(!source.contains("Fallback interface"));
        assert!(source.contains("module pkg_0::a"));
        assert!(source.contains("fun friend_only"));
        assert!(manifest.contains("name = \"pkg_0\""));
        assert!(manifest.contains("pkg_0 = \"0x0\""));
        assert!(artifact
            .raw_root
            .join("pkg_0/bytecode_modules/a.mv")
            .is_file());
    }

    #[test]
    fn raw_import_does_not_generate_buildable_artifact() {
        let temp = tempdir().expect("tempdir");
        let package = fetched_fixture_package("friend_function", "0x0");
        let request = BuildableImportRequest {
            network_id: "testnet".to_string(),
            graph_ql_url: "https://graphql.testnet.sui.io/graphql".to_string(),
            package_id: "0x0".to_string(),
            import_root: temp.path().join("import"),
            generate_buildable: false,
        };

        let artifact = disabled_engine()
            .migrate_fetched_packages(request, vec![package])
            .expect("migrate fixture");

        assert_eq!(artifact.project_root, artifact.raw_root.join("pkg_0"));
        assert!(artifact.project_root.join("Move.toml").is_file());
        assert!(artifact.project_root.join("sources/a.move").is_file());
        let source =
            fs::read_to_string(artifact.project_root.join("sources/a.move")).expect("source");
        assert!(source.contains("module pkg_0::a"));
        assert!(artifact
            .project_root
            .join("bytecode_modules/a.mv")
            .is_file());
        assert!(artifact
            .project_root
            .join("build/pkg_0/bytecode_modules/a.mv")
            .is_file());
        assert!(artifact
            .project_root
            .join("build/pkg_0/sources/a.move")
            .is_file());
        assert!(artifact.raw_root.join("pkg_0").is_dir());
        assert!(!artifact.raw_root.join("raw").exists());
        assert!(!artifact.buildable_root.exists());
        assert!(artifact.build_result.is_none());
    }

    #[test]
    fn aliases_use_last_eight_package_id_hex_characters() {
        let root_package_id =
            "0x53eedd7e88c454de73d0edea1aaeba07218cd5108fde2f07d3a987a140c10d80".to_string();
        let dependency_id =
            "0xf5ea2b3749c65d6e56507cc35388719aadb28f9cab873696a2f8687f5c785138".to_string();
        let package_ids = vec![root_package_id.clone(), dependency_id.clone()];
        let aliases = address_aliases(&root_package_id, package_ids.iter()).expect("aliases");

        let root_alias = alias_for_address(&aliases, &root_package_id).expect("root alias");
        let dependency_alias =
            alias_for_address(&aliases, &dependency_id).expect("dependency alias");

        assert_eq!(root_alias.alias, "pkg_40c10d80");
        assert_eq!(dependency_alias.alias, "dep_5c785138");
    }

    #[test]
    fn raw_manifest_keeps_dependency_aliases_without_dependency_entries() {
        let aliases = vec![
            AddressAlias {
                address: "0x1".to_string(),
                hex: "1".to_string(),
                alias: "std".to_string(),
                kind: AliasKind::Framework,
            },
            AddressAlias {
                address: "0x2".to_string(),
                hex: "2".to_string(),
                alias: "sui".to_string(),
                kind: AliasKind::Framework,
            },
            AddressAlias {
                address: "0xabc".to_string(),
                hex: "abc".to_string(),
                alias: "pkg_abc".to_string(),
                kind: AliasKind::Root,
            },
            AddressAlias {
                address: "0xdef".to_string(),
                hex: "def".to_string(),
                alias: "dep_def".to_string(),
                kind: AliasKind::Dependency,
            },
        ];
        let manifest = raw_package_manifest_source(&aliases[2], &aliases);

        assert!(!manifest.contains("[dependencies]"));
        assert!(!manifest.contains("dep_def = { local ="));
        assert!(manifest.contains("dep_def = \"0xdef\""));
        assert!(manifest.contains("edition = \"2024\""));
        assert!(!manifest.contains("flavor ="));
        assert!(manifest.contains("std = \"0x1\""));
        assert!(manifest.contains("sui = \"0x2\""));
        assert!(!manifest.contains("MoveStdlib ="));
        assert!(!manifest.contains("Sui ="));
    }

    #[test]
    fn manifest_wires_local_dependency_paths() {
        let aliases = vec![
            AddressAlias {
                address: "0x1".to_string(),
                hex: "1".to_string(),
                alias: "std".to_string(),
                kind: AliasKind::Framework,
            },
            AddressAlias {
                address: "0x2".to_string(),
                hex: "2".to_string(),
                alias: "sui".to_string(),
                kind: AliasKind::Framework,
            },
            AddressAlias {
                address: "0x3".to_string(),
                hex: "3".to_string(),
                alias: "sui_system".to_string(),
                kind: AliasKind::Framework,
            },
            AddressAlias {
                address: "0xb".to_string(),
                hex: "b".to_string(),
                alias: "bridge".to_string(),
                kind: AliasKind::Framework,
            },
            AddressAlias {
                address: "0xabc".to_string(),
                hex: "abc".to_string(),
                alias: "pkg_abc".to_string(),
                kind: AliasKind::Root,
            },
            AddressAlias {
                address: "0xdef".to_string(),
                hex: "def".to_string(),
                alias: "dep_def".to_string(),
                kind: AliasKind::Dependency,
            },
        ];
        let dependencies = BTreeSet::from([
            "0x1".to_string(),
            "0x2".to_string(),
            "0x3".to_string(),
            "0xb".to_string(),
            "0xdef".to_string(),
        ]);
        let manifest = package_manifest_source(&aliases[4], &aliases, &dependencies, "2024")
            .expect("manifest");

        assert!(manifest.contains("dep_def = { local = \"deps/dep_def\" }"));
        assert!(manifest.contains("dep_def = \"0xdef\""));
        assert!(manifest.contains("published-at = \"0xabc\""));
        assert!(manifest.contains("edition = \"2024\""));
        assert!(!manifest.contains("flavor ="));
        assert!(manifest.contains("std = \"0x1\""));
        assert!(manifest.contains("sui = \"0x2\""));
        assert!(manifest.contains("sui_system = \"0x3\""));
        assert!(manifest.contains("bridge = \"0xb\""));
        assert!(!manifest.contains("MoveStdlib ="));
        assert!(!manifest.contains("Sui ="));
        assert!(!manifest.contains("SuiSystem ="));
        assert!(!manifest.contains("Bridge ="));
    }

    #[test]
    fn rewrites_full_and_bare_package_ids_to_named_addresses() {
        let aliases = vec![AddressAlias {
            address: "0xf5ea2b3749c65d6e56507cc35388719aadb28f9cab873696a2f8687f5c785138"
                .to_string(),
            hex: "f5ea2b3749c65d6e56507cc35388719aadb28f9cab873696a2f8687f5c785138".to_string(),
            alias: "pkg_f5ea2b37".to_string(),
            kind: AliasKind::Root,
        }];
        let source = "module f5ea2b3749c65d6e56507cc35388719aadb28f9cab873696a2f8687f5c785138::oracle;\npublic fun id(): 0xf5ea2b3749c65d6e56507cc35388719aadb28f9cab873696a2f8687f5c785138::oracle::T { abort 0 }\n";
        let rewritten = rewrite_source_addresses(source, &aliases).expect("rewrite");

        assert!(rewritten.contains("module pkg_f5ea2b37::oracle"));
        assert!(rewritten.contains("pkg_f5ea2b37::oracle::T"));
        assert!(
            !rewritten.contains("f5ea2b3749c65d6e56507cc35388719aadb28f9cab873696a2f8687f5c785138")
        );
    }

    #[test]
    fn creates_constants_module_with_accessor_functions_for_duplicate_literals() {
        let mut sources = BTreeMap::from([
            (
                "a".to_string(),
                "module pkg_0::a;\nconst C0: u64 = 42;\n".to_string(),
            ),
            (
                "b".to_string(),
                "module pkg_0::b;\nconst C1: u64 = 42;\n".to_string(),
            ),
        ]);

        migrate_shared_constants(&mut sources, "pkg_0").expect("constants");

        let constants = sources.get("constants").expect("constants source");
        assert!(constants.contains("module pkg_0::constants"));
        assert!(constants.contains("public fun shared_"));
        assert!(constants.contains("(): u64"));
        assert!(constants.contains("42"));
    }

    #[test]
    fn build_verification_can_build_migrated_fixture_package() {
        let temp = tempdir().expect("tempdir");
        let package = fetched_fixture_package("friend_function", "0x0");
        let request = BuildableImportRequest {
            network_id: "testnet".to_string(),
            graph_ql_url: "https://graphql.testnet.sui.io/graphql".to_string(),
            package_id: "0x0".to_string(),
            import_root: temp.path().join("import"),
            generate_buildable: true,
        };
        let engine = ImportEngine::new(ImportEngineConfig {
            build_verification: BuildVerification::SystemSui {
                executable: PathBuf::from("sui"),
                default_move_flavor: Some("core".to_string()),
            },
            ..ImportEngineConfig::default()
        });

        let artifact = engine
            .migrate_fetched_packages(request, vec![package])
            .expect("migrate fixture");
        let build_result = artifact.build_result.expect("build result");

        assert!(
            build_result.success,
            "stdout:\n{}\nstderr:\n{}",
            build_result.stdout, build_result.stderr
        );
    }

    #[tokio::test]
    #[ignore = "requires live GraphQL access and a package id in PEREGRINE_LIVE_PACKAGE_ID"]
    async fn live_graphql_import_migrates_and_builds_package() {
        let package_id =
            std::env::var("PEREGRINE_LIVE_PACKAGE_ID").expect("PEREGRINE_LIVE_PACKAGE_ID");
        let graph_ql_url = std::env::var("PEREGRINE_LIVE_GRAPHQL_URL")
            .unwrap_or_else(|_| "https://graphql.testnet.sui.io/graphql".to_string());
        let temp = tempdir().expect("tempdir");
        let request = BuildableImportRequest {
            network_id: "testnet".to_string(),
            graph_ql_url,
            package_id,
            import_root: temp.path().join("import"),
            generate_buildable: true,
        };

        let artifact = ImportEngine::default()
            .import_buildable_package(request)
            .await
            .expect("live import");

        assert!(artifact.buildable_root.join("Move.toml").is_file());
        assert!(
            artifact
                .build_result
                .as_ref()
                .map(|result| result.success)
                .unwrap_or(false),
            "{:?}",
            artifact.build_result
        );
    }
}
