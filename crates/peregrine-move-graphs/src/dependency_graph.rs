use super::{PackageDependencyEdge, PackageDependencyGraph, PackageDependencyNode};
use peregrine_move_model::{relative_path, root_package_name, MovePackageModel};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn build_package_dependency_graph(
    root: &Path,
    packages: &[MovePackageModel],
) -> PackageDependencyGraph {
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

fn resolve_summary_location(root: &Path, packages: &[MovePackageModel]) -> Option<SummaryLocation> {
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
        for (edge_source, edge_target) in edges.keys() {
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
