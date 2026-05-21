mod call_graph;
mod dependency_graph;
mod graph_builder;
mod state_access_graph;
mod type_graph;

use dependency_graph::build_package_dependency_graph;
use graph_builder::{build_move_graphs, build_move_state_access_graph, MoveStateAccessGraphTarget};
use peregrine_move_model::{
    build_move_package, discover_move_packages, root_package_name, MovePackageModel,
};
use serde::Serialize;
use std::path::Path;

pub use call_graph::{
    MoveCallGraph, MoveCallGraphEdge, MoveCallGraphNode, MoveSourceSpan, MoveUnresolvedCall,
};
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
