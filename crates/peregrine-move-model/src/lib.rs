mod call_graph;
mod dependency_graph;
mod graph_builder;
mod source_parser;
mod state_access_graph;
mod type_graph;

use dependency_graph::build_package_dependency_graph;
use graph_builder::{build_move_graphs, build_move_state_access_graph, MoveStateAccessGraphTarget};
use serde::Serialize;
use source_parser::discover_modules;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use call_graph::{
    MoveCallGraph, MoveCallGraphEdge, MoveCallGraphNode, MoveSourceSpan, MoveUnresolvedCall,
};
pub use source_parser::parse_module_declarations;
pub use state_access_graph::{
    MoveStateAccessGraph, MoveStateAccessGraphEdge, MoveStateAccessGraphNode,
    MoveUnresolvedStateAccess,
};
pub use type_graph::{
    MoveTypeGraph, MoveTypeGraphEdge, MoveTypeGraphNode, MoveTypeParameter, MoveUnresolvedType,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProjectModel {
    pub packages: Vec<MovePackageModel>,
    pub dependency_graph: PackageDependencyGraph,
    pub call_graph: MoveCallGraph,
    pub type_graph: MoveTypeGraph,
    pub state_access_graph: MoveStateAccessGraph,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProjectGraphs {
    pub call_graph: MoveCallGraph,
    pub type_graph: MoveTypeGraph,
    pub state_access_graph: MoveStateAccessGraph,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageModel {
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
    pub attributes: Vec<String>,
    pub structs: Vec<MoveStructSignature>,
    pub functions: Vec<MoveFunctionSignature>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructSignature {
    pub name: String,
    pub abilities: Vec<String>,
    pub fields: Vec<MoveStructField>,
    pub signature: String,
    pub attributes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructField {
    pub name: String,
    pub type_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionSignature {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub signature: String,
    pub body: Option<String>,
    pub attributes: Vec<String>,
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

pub fn discover_move_project_model(root: &Path) -> MoveProjectModel {
    discover_move_project_model_with_graphs(root, true)
}

pub fn discover_move_project_model_fast(root: &Path) -> MoveProjectModel {
    discover_move_project_model_with_graphs(root, false)
}

pub fn discover_move_project_model_shallow(root: &Path) -> MoveProjectModel {
    let packages = discover_move_packages(root, false);
    let dependency_graph = shallow_dependency_graph(&packages);

    MoveProjectModel {
        packages,
        dependency_graph,
        call_graph: MoveCallGraph::default(),
        type_graph: MoveTypeGraph::default(),
        state_access_graph: MoveStateAccessGraph::default(),
    }
}

fn shallow_dependency_graph(packages: &[MovePackageModel]) -> PackageDependencyGraph {
    let root = root_package_name(packages);

    PackageDependencyGraph {
        root: root.clone(),
        nodes: root
            .map(|id| {
                vec![PackageDependencyNode {
                    id,
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

pub fn discover_move_project_graphs(root: &Path) -> MoveProjectGraphs {
    let packages = discover_move_packages(root, false);
    let (call_graph, type_graph, state_access_graph) = build_move_graphs(root, &packages);

    MoveProjectGraphs {
        call_graph,
        type_graph,
        state_access_graph,
    }
}

pub fn discover_move_project_graphs_for_package(
    root: &Path,
    package_path: &str,
) -> MoveProjectGraphs {
    let manifest_path = root.join(package_path).join("Move.toml");
    let packages = build_move_package(root, &manifest_path, false)
        .map(|package| vec![package])
        .unwrap_or_else(|| discover_move_packages(root, false));
    let (call_graph, type_graph, state_access_graph) = build_move_graphs(root, &packages);

    MoveProjectGraphs {
        call_graph,
        type_graph,
        state_access_graph,
    }
}

pub fn discover_move_state_access_graph_for_function(
    root: &Path,
    package_path: &str,
    address: Option<String>,
    module_name: &str,
    function_name: &str,
) -> MoveStateAccessGraph {
    let manifest_path = root.join(package_path).join("Move.toml");
    let packages = build_move_package(root, &manifest_path, false)
        .map(|package| vec![package])
        .unwrap_or_else(|| discover_move_packages(root, false));

    build_move_state_access_graph(
        root,
        &packages,
        MoveStateAccessGraphTarget {
            package_path: package_path.to_string(),
            address,
            module_name: module_name.to_string(),
            function_name: function_name.to_string(),
            max_call_depth: 4,
        },
    )
}

fn discover_move_project_model_with_graphs(root: &Path, include_graphs: bool) -> MoveProjectModel {
    let packages = discover_move_packages(root, true);
    let dependency_graph = build_package_dependency_graph(root, &packages);
    let (call_graph, type_graph, state_access_graph) = if include_graphs {
        build_move_graphs(root, &packages)
    } else {
        (
            MoveCallGraph::default(),
            MoveTypeGraph::default(),
            MoveStateAccessGraph::default(),
        )
    };

    MoveProjectModel {
        packages,
        dependency_graph,
        call_graph,
        type_graph,
        state_access_graph,
    }
}

fn discover_move_packages(root: &Path, include_modules: bool) -> Vec<MovePackageModel> {
    let mut manifest_paths = Vec::new();

    collect_move_manifests(root, root, &mut manifest_paths);
    manifest_paths.sort();

    manifest_paths
        .into_iter()
        .filter_map(|manifest_path| build_move_package(root, &manifest_path, include_modules))
        .collect::<Vec<_>>()
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
            if should_skip_project_discovery_dir(&path) {
                continue;
            }

            collect_move_manifests(root, &path, manifest_paths);
            continue;
        }

        if file_type.is_file() && entry.file_name() == "Move.toml" && path.starts_with(root) {
            manifest_paths.push(path);
        }
    }
}

fn should_skip_project_discovery_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    matches!(
        name,
        ".git"
            | ".next"
            | ".sui"
            | ".turbo"
            | "build"
            | "coverage"
            | "dist"
            | "node_modules"
            | "package_summaries"
            | "target"
    )
}

pub(crate) fn root_package_name(packages: &[MovePackageModel]) -> Option<String> {
    packages
        .iter()
        .find(|move_package| move_package.path.is_empty())
        .or_else(|| packages.first())
        .map(|move_package| move_package.name.clone())
}

fn build_move_package(
    root: &Path,
    manifest_path: &Path,
    include_modules: bool,
) -> Option<MovePackageModel> {
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
    let mut modules = if include_modules {
        discover_modules(root, package_root)
    } else {
        Vec::new()
    };

    modules.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });

    Some(MovePackageModel {
        name,
        path,
        manifest_path,
        modules,
    })
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

pub(crate) fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn state_access_graph_tracks_state_types_and_fields_from_ast() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            r#"
[package]
name = "state_pkg"
"#,
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("sources/vault.move"),
            r#"
module state_pkg::vault;

public struct Vault has key, store {
    id: UID,
    balance: u64,
}

public fun deposit(vault: &mut Vault, amount: u64) {
    vault.balance = vault.balance + amount;
}

public fun route(vault: &mut Vault, amount: u64) {
    deposit(vault, amount);
}
"#,
        )
        .expect("module");

        let project = discover_move_project_model(temp.path());
        let graph = project.state_access_graph;
        let deposit_id = "function::state_pkg::vault::deposit";
        let route_id = "function::state_pkg::vault::route";
        let vault_type = graph
            .nodes
            .iter()
            .find(|node| node.qualified_name == "state_pkg::vault::Vault")
            .expect("Vault state type");
        let balance_field = graph
            .nodes
            .iter()
            .find(|node| node.qualified_name == "state_pkg::vault::Vault.balance")
            .expect("Vault.balance state field");

        assert!(graph.edges.iter().any(|edge| {
            edge.source == deposit_id
                && edge.target == vault_type.id
                && edge.access_kind == "borrowMut"
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.source == deposit_id
                && edge.target == balance_field.id
                && edge.access_kind == "write"
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.source == route_id && edge.target == deposit_id && edge.access_kind == "call"
        }));
    }
}
