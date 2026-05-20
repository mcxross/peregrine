use crate::file_preview::{build_file_preview, FilePreview};
use crate::validated_move_project_name;
use base64::{engine::general_purpose, Engine};
use peregrine_static_analysis::sui::bytecode_view::{
    load_package_bytecode, MoveBytecodePackageView,
};
use peregrine_static_analysis::{
    discover_move_project_fast, discover_move_project_shallow, discover_project_graphs,
    discover_project_graphs_for_package, discover_state_access_graph_for_function, AnalysisConfig,
    AnalysisEngine, AnalysisEngineOptions, AnalysisReport, MoveCallGraph, MovePackage,
    MoveProjectGraphs, MoveStateAccessGraph, MoveTypeGraph, PackageDependencyGraph,
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageTree {
    root_path: String,
    root_name: String,
    is_detailed: bool,
    paths: Vec<String>,
    move_packages: Vec<MovePackage>,
    dependency_graph: PackageDependencyGraph,
    call_graph: MoveCallGraph,
    type_graph: MoveTypeGraph,
    state_access_graph: MoveStateAccessGraph,
}

#[tauri::command]
pub(crate) async fn load_package_tree(root_path: String) -> Result<PackageTree, String> {
    tauri::async_runtime::spawn_blocking(move || {
        build_package_tree(root_path, PackageTreeMode::Shallow)
    })
    .await
    .map_err(|error| format!("Could not join package tree task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_package_tree_details(root_path: String) -> Result<PackageTree, String> {
    tauri::async_runtime::spawn_blocking(move || {
        build_package_tree(root_path, PackageTreeMode::Detailed)
    })
    .await
    .map_err(|error| format!("Could not join package detail task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_move_graphs(
    root_path: String,
    package_path: Option<String>,
) -> Result<MoveProjectGraphs, String> {
    tauri::async_runtime::spawn_blocking(move || build_move_graphs(root_path, package_path))
        .await
        .map_err(|error| format!("Could not join Move graph task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_move_state_access_graph(
    root_path: String,
    package_path: String,
    module_address: Option<String>,
    module_name: String,
    function_name: String,
) -> Result<MoveStateAccessGraph, String> {
    tauri::async_runtime::spawn_blocking(move || {
        build_move_state_access_graph(
            root_path,
            package_path,
            module_address,
            module_name,
            function_name,
        )
    })
    .await
    .map_err(|error| format!("Could not join Move state graph task: {error}"))?
}

#[tauri::command]
pub(crate) async fn move_project_path_exists(
    parent_path: String,
    project_name: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let project_name = match validated_move_project_name(&project_name) {
            Ok(project_name) => project_name,
            Err(_) => return Ok(false),
        };
        let parent = PathBuf::from(&parent_path)
            .canonicalize()
            .map_err(|error| format!("Could not read parent directory {parent_path}: {error}"))?;

        if !parent.is_dir() {
            return Err("Project parent path is not a directory.".to_string());
        }

        Ok(parent.join(project_name).exists())
    })
    .await
    .map_err(|error| format!("Could not join Move project path check task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_file_preview(
    root_path: String,
    relative_path: String,
) -> Result<FilePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file_path = resolve_package_child_path(&root_path, &relative_path)?;
        build_file_preview(&file_path, relative_path)
    })
    .await
    .map_err(|error| format!("Could not join file preview task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_text_file(
    root_path: String,
    relative_path: String,
    contents: String,
) -> Result<FilePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file_path = resolve_package_child_path(&root_path, &relative_path)?;
        fs::write(&file_path, contents)
            .map_err(|error| format!("Could not write {}: {error}", file_path.display()))?;
        build_file_preview(&file_path, relative_path)
    })
    .await
    .map_err(|error| format!("Could not join file save task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_graph_png(path: String, png_data_url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let encoded = png_data_url
            .strip_prefix("data:image/png;base64,")
            .ok_or_else(|| "Expected a PNG data URL.".to_string())?;
        let bytes = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| format!("Could not decode graph PNG: {error}"))?;

        fs::write(&path, bytes).map_err(|error| format!("Could not write {path}: {error}"))
    })
    .await
    .map_err(|error| format!("Could not join graph image save task: {error}"))?
}

#[tauri::command]
pub(crate) async fn analyze_move_package(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
) -> Result<AnalysisReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let package_root = resolve_package_child_path(&root_path, &package_path)?;

        if !package_root.is_dir() {
            return Err("Selected package path is not a directory.".to_string());
        }

        if !package_root.join("Move.toml").is_file() {
            return Err("Selected package does not contain a Move.toml file.".to_string());
        }

        let config = AnalysisConfig::load_from_package(&package_root)?;
        let registry_root = app
            .path()
            .app_config_dir()
            .map_err(|error| format!("Could not resolve app config directory: {error}"))?;

        Ok(AnalysisEngine::new().analyze_package_with_options(
            package_root,
            config,
            AnalysisEngineOptions {
                global_plugin_root: Some(registry_root),
                ..AnalysisEngineOptions::default()
            },
        ))
    })
    .await
    .map_err(|error| format!("Could not join Move analysis task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_move_bytecode_view(
    root_path: String,
    package_path: String,
    package_name: String,
) -> Result<MoveBytecodePackageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let package_root = resolve_package_child_path(&root_path, &package_path)?;

        if !package_root.is_dir() {
            return Err("Selected package path is not a directory.".to_string());
        }

        if !package_root.join("Move.toml").is_file() {
            return Err("Selected package does not contain a Move.toml file.".to_string());
        }

        load_package_bytecode(package_root, &package_name)
    })
    .await
    .map_err(|error| format!("Could not join bytecode view task: {error}"))?
}

pub(crate) enum PackageTreeMode {
    Detailed,
    Shallow,
}

pub(crate) fn build_package_tree(
    root_path: String,
    mode: PackageTreeMode,
) -> Result<PackageTree, String> {
    let root = PathBuf::from(&root_path);
    let root = root
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(root_path.as_str())
        .to_string();
    let mut paths = Vec::new();

    collect_paths(&root, &root, &mut paths)?;
    paths.sort_by(|left, right| compare_tree_paths(left, right));

    let move_project = match mode {
        PackageTreeMode::Detailed => discover_move_project_fast(&root),
        PackageTreeMode::Shallow => discover_move_project_shallow(&root),
    };

    Ok(PackageTree {
        root_path: root.to_string_lossy().into_owned(),
        root_name,
        is_detailed: matches!(mode, PackageTreeMode::Detailed),
        paths,
        move_packages: move_project.packages,
        dependency_graph: move_project.dependency_graph,
        call_graph: move_project.call_graph,
        type_graph: move_project.type_graph,
        state_access_graph: move_project.state_access_graph,
    })
}

fn build_move_state_access_graph(
    root_path: String,
    package_path: String,
    module_address: Option<String>,
    module_name: String,
    function_name: String,
) -> Result<MoveStateAccessGraph, String> {
    let root = PathBuf::from(&root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;
    let package_root = root.join(&package_path).canonicalize().map_err(|error| {
        format!(
            "Could not read Move package {}: {error}",
            root.join(&package_path).display()
        )
    })?;

    if !package_root.starts_with(&root) {
        return Err("Move package must be inside the opened project.".to_string());
    }

    Ok(discover_state_access_graph_for_function(
        &root,
        &package_path,
        module_address,
        &module_name,
        &function_name,
    ))
}

fn build_move_graphs(
    root_path: String,
    package_path: Option<String>,
) -> Result<MoveProjectGraphs, String> {
    let root = PathBuf::from(&root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;

    if let Some(package_path) = package_path {
        let package_root = root.join(&package_path).canonicalize().map_err(|error| {
            format!(
                "Could not read Move package {}: {error}",
                root.join(&package_path).display()
            )
        })?;

        if !package_root.starts_with(&root) {
            return Err("Move package must be inside the opened project.".to_string());
        }

        return Ok(discover_project_graphs_for_package(&root, &package_path));
    }

    Ok(discover_project_graphs(&root))
}

fn collect_paths(root: &Path, directory: &Path, paths: &mut Vec<String>) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if directory == root => {
            return Err(format!(
                "Could not read package directory {}: {error}",
                directory.display()
            ));
        }
        Err(_) => return Ok(()),
    };

    let mut entries = entries
        .filter_map(Result::ok)
        .collect::<Vec<fs::DirEntry>>();
    entries.sort_by(compare_dir_entries);

    for entry in entries {
        let path = entry.path();
        let Ok(relative_path) = path.strip_prefix(root) else {
            continue;
        };
        let Some(relative_path) = normalize_tree_path(relative_path) else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            paths.push(relative_path);
            continue;
        };

        if file_type.is_dir() {
            if should_skip_tree_directory(&path) {
                continue;
            }

            paths.push(format!("{relative_path}/"));
            collect_paths(root, &path, paths)?;
        } else {
            paths.push(relative_path);
        }
    }

    Ok(())
}

fn should_skip_tree_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    matches!(
        name,
        ".git"
            | ".next"
            | ".peregrine"
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

fn compare_dir_entries(left: &fs::DirEntry, right: &fs::DirEntry) -> std::cmp::Ordering {
    let left_is_dir = left
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or(false);
    let right_is_dir = right
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or(false);

    right_is_dir
        .cmp(&left_is_dir)
        .then_with(|| left.file_name().cmp(&right.file_name()))
}

fn compare_tree_paths(left: &str, right: &str) -> std::cmp::Ordering {
    let left_is_dir = left.ends_with('/');
    let right_is_dir = right.ends_with('/');

    right_is_dir.cmp(&left_is_dir).then_with(|| left.cmp(right))
}

fn normalize_tree_path(path: &Path) -> Option<String> {
    Some(
        path.components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}

pub(crate) fn resolve_package_child_path(
    root_path: &str,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let root = PathBuf::from(root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;
    let file_path = root.join(relative_path.trim_end_matches('/'));
    let canonical_file_path = file_path
        .canonicalize()
        .map_err(|error| format!("Could not resolve {}: {error}", file_path.display()))?;

    if !canonical_file_path.starts_with(&root) {
        return Err("Selected file is outside of the package directory.".to_string());
    }

    Ok(canonical_file_path)
}
