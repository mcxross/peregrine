use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackage {
    pub name: String,
    pub path: String,
    pub manifest_path: String,
    pub modules: Vec<MoveModule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveModule {
    pub name: String,
    pub address: Option<String>,
    pub file_path: String,
    pub functions: Vec<MoveFunctionSignature>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionSignature {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyGraph {
    pub root: Option<String>,
    pub nodes: Vec<PackageDependencyNode>,
    pub edges: Vec<PackageDependencyEdge>,
    pub summary_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyNode {
    pub id: String,
    pub address: Option<String>,
    pub module_count: usize,
    pub public_function_count: usize,
    pub entry_function_count: usize,
    pub is_root: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyEdge {
    pub source: String,
    pub target: String,
    pub dependency_count: usize,
    pub dependency_kind: String,
}

pub fn discover_move_project(root: &Path) -> (Vec<MovePackage>, PackageDependencyGraph) {
    let mut manifest_paths = Vec::new();

    collect_move_manifests(root, root, &mut manifest_paths);
    manifest_paths.sort();

    let packages = manifest_paths
        .into_iter()
        .filter_map(|manifest_path| build_move_package(root, &manifest_path))
        .collect::<Vec<_>>();
    let graph = build_package_dependency_graph(root, &packages);

    (packages, graph)
}

fn collect_move_manifests(root: &Path, directory: &Path, manifest_paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_manifests(root, &path, manifest_paths);
            continue;
        }

        if file_type.is_file() && entry.file_name() == "Move.toml" && path.starts_with(root) {
            manifest_paths.push(path);
        }
    }
}

fn root_package_name(packages: &[MovePackage]) -> Option<String> {
    packages
        .iter()
        .find(|move_package| move_package.path.is_empty())
        .or_else(|| packages.first())
        .map(|move_package| move_package.name.clone())
}

fn build_move_package(root: &Path, manifest_path: &Path) -> Option<MovePackage> {
    let package_root = manifest_path.parent()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    let name = package_name(&manifest).unwrap_or_else(|| {
        package_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Move package")
            .to_string()
    });
    let path = relative_path(root, package_root)?;
    let manifest_path = relative_path(root, manifest_path)?;
    let mut modules = discover_modules(root, package_root);

    modules.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });

    Some(MovePackage {
        name,
        path,
        manifest_path,
        modules,
    })
}

fn discover_modules(root: &Path, package_root: &Path) -> Vec<MoveModule> {
    let sources = package_root.join("sources");
    let mut modules = Vec::new();

    collect_move_modules(root, &sources, &mut modules);
    modules
}

fn collect_move_modules(root: &Path, directory: &Path, modules: &mut Vec<MoveModule>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_modules(root, &path, modules);
            continue;
        }

        if !file_type.is_file() || !is_move_file(&path) {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(module) = parse_module_declaration(&source, root, &path) else {
            continue;
        };

        modules.push(module);
    }
}

fn parse_module_declaration(source: &str, root: &Path, path: &Path) -> Option<MoveModule> {
    for line in source.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        let declaration = line
            .strip_prefix("module ")
            .or_else(|| line.strip_prefix("public module "))?;
        let qualified_name = declaration
            .split(|character: char| character == '{' || character.is_whitespace())
            .next()?
            .trim_end_matches(';');
        let (address, name) = match qualified_name.split_once("::") {
            Some((address, name)) => (Some(address.to_string()), name.to_string()),
            None => (None, qualified_name.to_string()),
        };

        if name.is_empty() {
            return None;
        }

        return Some(MoveModule {
            name,
            address,
            file_path: relative_path(root, path)?,
            functions: collect_function_signatures(source),
        });
    }

    None
}

fn collect_function_signatures(source: &str) -> Vec<MoveFunctionSignature> {
    let mut functions = Vec::new();
    let mut current = String::new();
    let mut is_collecting = false;

    for raw_line in source.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();

        if line.is_empty() {
            continue;
        }

        if !is_collecting && !line_contains_function_declaration(line) {
            continue;
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(line);
        is_collecting = true;

        if !line.contains('{') && !line.ends_with(';') {
            continue;
        }

        if let Some(signature) = parse_function_signature(&current) {
            functions.push(signature);
        }

        current.clear();
        is_collecting = false;
    }

    functions
}

fn line_contains_function_declaration(line: &str) -> bool {
    line == "fun"
        || line.starts_with("fun ")
        || line.contains(" fun ")
        || line.starts_with("public fun ")
        || line.starts_with("public entry fun ")
        || line.starts_with("entry fun ")
        || line.starts_with("public(package) fun ")
        || line.starts_with("public(friend) fun ")
}

fn parse_function_signature(source: &str) -> Option<MoveFunctionSignature> {
    let signature = source
        .split('{')
        .next()
        .unwrap_or(source)
        .trim()
        .trim_end_matches(';')
        .to_string();
    let fun_index = signature.find("fun ")?;
    let prefix = signature[..fun_index].trim();
    let after_fun = signature[fun_index + 4..].trim();
    let name = after_fun
        .split(|character: char| character == '<' || character == '(' || character.is_whitespace())
        .next()?
        .to_string();

    if name.is_empty() {
        return None;
    }

    Some(MoveFunctionSignature {
        name,
        visibility: function_visibility(prefix),
        is_entry: prefix.split_whitespace().any(|token| token == "entry"),
        signature,
    })
}

fn function_visibility(prefix: &str) -> String {
    if prefix.contains("public(friend)") {
        "public(friend)".to_string()
    } else if prefix.contains("public(package)") {
        "public(package)".to_string()
    } else if prefix.split_whitespace().any(|token| token == "public") {
        "public".to_string()
    } else {
        "private".to_string()
    }
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package_section = false;

    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();

        if line.starts_with('[') && line.ends_with(']') {
            in_package_section = line == "[package]";
            continue;
        }

        if !in_package_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if key.trim() != "name" {
            continue;
        }

        return Some(
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        );
    }

    None
}

fn build_package_dependency_graph(root: &Path, packages: &[MovePackage]) -> PackageDependencyGraph {
    let Some(summary_location) = resolve_summary_location(root, packages) else {
        return empty_dependency_graph(root_package_name(packages).as_deref());
    };

    let address_mapping = read_address_mapping(&summary_location.path.join("address_mapping.json"));
    let mut aggregate = DependencyAggregate::default();

    collect_summary_dependencies(&summary_location.path, &mut aggregate);

    let root = Some(summary_location.root_package.clone())
        .or_else(|| aggregate.module_counts.keys().next().cloned());
    let Some(root_id) = root else {
        return PackageDependencyGraph {
            root: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            summary_path: Some(summary_location.relative_path),
        };
    };

    let reachable = reachable_packages(&root_id, &aggregate.edges);
    let mut nodes = reachable
        .iter()
        .map(|id| PackageDependencyNode {
            id: id.clone(),
            address: address_mapping.get(id).cloned(),
            module_count: *aggregate.module_counts.get(id).unwrap_or(&0),
            public_function_count: *aggregate.public_function_counts.get(id).unwrap_or(&0),
            entry_function_count: *aggregate.entry_function_counts.get(id).unwrap_or(&0),
            is_root: id == &root_id,
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        right
            .is_root
            .cmp(&left.is_root)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut edges = aggregate
        .edges
        .into_iter()
        .filter(|((source, target), _)| reachable.contains(source) && reachable.contains(target))
        .map(
            |((source, target), dependency_count)| PackageDependencyEdge {
                source,
                target,
                dependency_count,
                dependency_kind: "Immediate module dependency".to_string(),
            },
        )
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
    });

    PackageDependencyGraph {
        root: Some(root_id),
        nodes,
        edges,
        summary_path: Some(summary_location.relative_path),
    }
}

struct SummaryLocation {
    path: PathBuf,
    relative_path: String,
    root_package: String,
}

fn resolve_summary_location(root: &Path, packages: &[MovePackage]) -> Option<SummaryLocation> {
    let root_summary = root.join("package_summaries");

    if root_summary.is_dir() {
        return Some(SummaryLocation {
            relative_path: relative_path(root, &root_summary)?,
            path: root_summary,
            root_package: root_package_name(packages)?,
        });
    }

    packages
        .iter()
        .filter_map(|move_package| {
            let package_summary = root.join(&move_package.path).join("package_summaries");

            if !package_summary.is_dir() {
                return None;
            }

            Some(SummaryLocation {
                relative_path: relative_path(root, &package_summary)?,
                path: package_summary,
                root_package: move_package.name.clone(),
            })
        })
        .min_by(|left, right| {
            left.relative_path
                .matches('/')
                .count()
                .cmp(&right.relative_path.matches('/').count())
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        })
}

fn empty_dependency_graph(root: Option<&str>) -> PackageDependencyGraph {
    PackageDependencyGraph {
        root: root.map(ToOwned::to_owned),
        nodes: root
            .map(|id| {
                vec![PackageDependencyNode {
                    id: id.to_string(),
                    address: None,
                    module_count: 0,
                    public_function_count: 0,
                    entry_function_count: 0,
                    is_root: true,
                }]
            })
            .unwrap_or_default(),
        edges: Vec::new(),
        summary_path: None,
    }
}

#[derive(Default)]
struct DependencyAggregate {
    module_counts: HashMap<String, usize>,
    public_function_counts: HashMap<String, usize>,
    entry_function_counts: HashMap<String, usize>,
    edges: HashMap<(String, String), usize>,
}

#[derive(Deserialize)]
struct ModuleSummary {
    id: ModuleId,
    #[serde(default)]
    immediate_dependencies: Vec<ModuleId>,
    #[serde(default)]
    functions: HashMap<String, SummaryFunction>,
}

#[derive(Deserialize)]
struct ModuleId {
    address: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Deserialize)]
struct SummaryFunction {
    #[serde(default)]
    visibility: Option<serde_json::Value>,
    #[serde(default)]
    entry: bool,
}

fn collect_summary_dependencies(directory: &Path, aggregate: &mut DependencyAggregate) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_summary_dependencies(&path, aggregate);
            continue;
        }

        if !file_type.is_file()
            || path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "address_mapping.json")
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(summary) = serde_json::from_str::<ModuleSummary>(&source) else {
            continue;
        };

        *aggregate
            .module_counts
            .entry(summary.id.address.clone())
            .or_default() += 1;

        for function in summary.functions.values() {
            if function.entry {
                *aggregate
                    .entry_function_counts
                    .entry(summary.id.address.clone())
                    .or_default() += 1;
            }

            if is_public_visibility(function.visibility.as_ref()) {
                *aggregate
                    .public_function_counts
                    .entry(summary.id.address.clone())
                    .or_default() += 1;
            }
        }

        for dependency in summary.immediate_dependencies {
            if dependency.address == summary.id.address {
                continue;
            }

            *aggregate
                .edges
                .entry((summary.id.address.clone(), dependency.address))
                .or_default() += 1;
        }
    }
}

fn is_public_visibility(visibility: Option<&serde_json::Value>) -> bool {
    match visibility {
        Some(serde_json::Value::String(value)) => value.starts_with("Public"),
        Some(serde_json::Value::Object(value)) => value.contains_key("Public"),
        _ => false,
    }
}

fn reachable_packages(root: &str, edges: &HashMap<(String, String), usize>) -> HashSet<String> {
    let mut reachable = HashSet::from([root.to_string()]);
    let mut queue = VecDeque::from([root.to_string()]);

    while let Some(source) = queue.pop_front() {
        for ((edge_source, edge_target), _) in edges {
            if edge_source != &source || reachable.contains(edge_target) {
                continue;
            }

            reachable.insert(edge_target.clone());
            queue.push_back(edge_target.clone());
        }
    }

    reachable
}

fn read_address_mapping(path: &Path) -> HashMap<String, String> {
    let Ok(source) = fs::read_to_string(path) else {
        return HashMap::new();
    };

    serde_json::from_str(&source).unwrap_or_default()
}

fn is_move_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("move"))
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}
