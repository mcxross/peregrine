use crate::file_preview::{FilePreview, build_file_preview};
use crate::validated_move_project_name;
use base64::{Engine, engine::general_purpose};
use peregrine_analysis::{AnalysisRequest, AnalysisStage, AnalysisTarget, ChainId, GraphKind};
use peregrine_sui_adapter::SuiAdapterSettings;
use peregrine_sui_bytecode::{MoveBytecodePackageView, load_package_bytecode};
use peregrine_sui_move_graph::{MoveProjectGraphs, MoveStateAccessGraph};
use peregrine_sui_move_model::{MovePackageModel, build_move_package};
use peregrine_sui_project_loader::{
    LoadedProject, ProjectLoadMode, ProjectLoadOptions, legacy_move_project_graphs,
    legacy_state_access_graph, legacy_static_report, load_project, run_sui_analysis_blocking,
};
use peregrine_sui_static_analysis::AnalysisReport;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::Manager;

pub(crate) type PackageTree = LoadedProject;

#[tauri::command]
pub(crate) async fn load_package_tree(root_path: String) -> Result<PackageTree, String> {
    let options = project_load_options(None, PackageTreeMode::Shallow);

    tauri::async_runtime::spawn_blocking(move || {
        build_package_tree_with_options(root_path, options)
    })
    .await
    .map_err(|error| format!("Could not join package tree task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_package_tree_details(
    app: tauri::AppHandle,
    root_path: String,
) -> Result<PackageTree, String> {
    let options = project_load_options(Some(&app), PackageTreeMode::Detailed);

    tauri::async_runtime::spawn_blocking(move || {
        build_package_tree_with_options(root_path, options)
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
    include_highlighted_html: Option<bool>,
) -> Result<FilePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file_path = resolve_package_child_path(&root_path, &relative_path)?;
        build_file_preview(
            &file_path,
            relative_path,
            include_highlighted_html.unwrap_or(true),
        )
    })
    .await
    .map_err(|error| format!("Could not join file preview task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_text_file(
    root_path: String,
    relative_path: String,
    contents: String,
    include_highlighted_html: Option<bool>,
) -> Result<FilePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file_path = resolve_package_child_write_path(&root_path, &relative_path)?;
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        fs::write(&file_path, contents)
            .map_err(|error| format!("Could not write {}: {error}", file_path.display()))?;
        build_file_preview(
            &file_path,
            relative_path,
            include_highlighted_html.unwrap_or(true),
        )
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
pub(crate) async fn save_text_export(path: String, contents: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        fs::write(&path, contents).map_err(|error| format!("Could not write {path}: {error}"))
    })
    .await
    .map_err(|error| format!("Could not join text export save task: {error}"))?
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

        let registry_root = app
            .path()
            .app_config_dir()
            .map_err(|error| format!("Could not resolve app config directory: {error}"))?;
        let mut request = AnalysisRequest::safe(
            ChainId::new("sui"),
            AnalysisTarget::LocalPackage { path: package_root },
        );
        request.options.insert(
            "globalPluginRoot".to_string(),
            json!(registry_root.to_string_lossy()),
        );
        let report = run_sui_analysis_blocking(request, SuiAdapterSettings::default())?;
        Ok(legacy_static_report(&report))
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
        let root = PathBuf::from(&root_path)
            .canonicalize()
            .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;
        let (package_root, package) = resolve_move_package(&root, &package_path)?;

        require_move_source_modules(&package)?;

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
    build_package_tree_with_options(root_path, project_load_options(None, mode))
}

fn build_package_tree_with_options(
    root_path: String,
    options: ProjectLoadOptions,
) -> Result<PackageTree, String> {
    load_project(root_path, options)
}

fn project_load_options(
    app: Option<&tauri::AppHandle>,
    mode: PackageTreeMode,
) -> ProjectLoadOptions {
    let analyzer_plugin_root = app.and_then(|app| app.path().app_config_dir().ok());

    ProjectLoadOptions {
        analyzer_plugin_root,
        include_analyzer: matches!(mode, PackageTreeMode::Detailed),
        mode: match mode {
            PackageTreeMode::Detailed => ProjectLoadMode::Detailed,
            PackageTreeMode::Shallow => ProjectLoadMode::Shallow,
        },
    }
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
    let (package_root, package) = resolve_move_package(&root, &package_path)?;

    require_move_source_modules(&package)?;

    let mut request = AnalysisRequest::safe(
        ChainId::new("sui"),
        AnalysisTarget::LocalPackage { path: package_root },
    );
    request.stages = vec![AnalysisStage::Scan, AnalysisStage::Graph];
    request.graph_kinds = vec![GraphKind::new(GraphKind::STATE_ACCESS)];
    request
        .options
        .insert("packagePath".to_string(), json!("."));
    request
        .options
        .insert("moduleName".to_string(), json!(module_name));
    request
        .options
        .insert("functionName".to_string(), json!(function_name));
    if let Some(module_address) = module_address {
        request
            .options
            .insert("address".to_string(), json!(module_address));
    }
    let report = run_sui_analysis_blocking(request, SuiAdapterSettings::default())?;
    legacy_state_access_graph(&report)
}

fn build_move_graphs(
    root_path: String,
    package_path: Option<String>,
) -> Result<MoveProjectGraphs, String> {
    let root = PathBuf::from(&root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;

    let package_root = if let Some(package_path) = package_path {
        let (package_root, package) = resolve_move_package(&root, &package_path)?;

        require_move_source_modules(&package)?;
        package_root
    } else {
        root
    };
    let mut request = AnalysisRequest::safe(
        ChainId::new("sui"),
        AnalysisTarget::LocalPackage { path: package_root },
    );
    request.stages = vec![AnalysisStage::Scan, AnalysisStage::Graph];
    request.graph_kinds = [GraphKind::CALL, GraphKind::TYPE, GraphKind::STATE_ACCESS]
        .into_iter()
        .map(GraphKind::new)
        .collect();
    let report = run_sui_analysis_blocking(request, SuiAdapterSettings::default())?;
    legacy_move_project_graphs(&report)
}

fn resolve_move_package(
    root: &Path,
    package_path: &str,
) -> Result<(PathBuf, MovePackageModel), String> {
    let package_root = root.join(package_path).canonicalize().map_err(|error| {
        format!(
            "Could not read Move package {}: {error}",
            root.join(package_path).display()
        )
    })?;

    if !package_root.starts_with(root) {
        return Err("Move package must be inside the opened project.".to_string());
    }

    let manifest_path = package_root.join("Move.toml");

    if !manifest_path.is_file() {
        return Err("Selected package does not contain a Move.toml file.".to_string());
    }

    let package = build_move_package(root, &manifest_path, false)
        .ok_or_else(|| "Could not read selected Move package manifest.".to_string())?;

    Ok((package_root, package))
}

fn require_move_source_modules(package: &MovePackageModel) -> Result<(), String> {
    if package.has_source_modules {
        return Ok(());
    }

    Err(move_source_unavailable_message(package))
}

fn move_source_unavailable_message(package: &MovePackageModel) -> String {
    let path = if package.path.is_empty() {
        "."
    } else {
        package.path.as_str()
    };

    if package.source_file_count == 0 {
        return format!(
            "Move package `{}` ({path}) contains a Move.toml manifest but no Move source files under sources/. Dependency graph, call graph, type graph, and bytecode views require parseable source modules.",
            package.name
        );
    }

    format!(
        "Move package `{}` ({path}) contains {} Move source {}, but no parseable Move modules were found. The source may be commented out or invalid. Dependency graph, call graph, type graph, and bytecode views require parseable source modules.",
        package.name,
        package.source_file_count,
        if package.source_file_count == 1 {
            "file"
        } else {
            "files"
        }
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

fn resolve_package_child_write_path(
    root_path: &str,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let root = PathBuf::from(root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;
    let relative = Path::new(relative_path.trim_end_matches('/'));

    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("Selected file is outside of the package directory.".to_string());
    }

    let file_path = root.join(relative);
    let Some(parent) = file_path.parent() else {
        return Err("Selected file has no parent directory.".to_string());
    };
    let existing_parent = nearest_existing_parent(parent)?;
    let canonical_parent = existing_parent
        .canonicalize()
        .map_err(|error| format!("Could not resolve {}: {error}", existing_parent.display()))?;

    if !canonical_parent.starts_with(&root) {
        return Err("Selected file is outside of the package directory.".to_string());
    }

    Ok(file_path)
}

fn nearest_existing_parent(path: &Path) -> Result<PathBuf, String> {
    let mut candidate = path;

    loop {
        if candidate.exists() {
            return Ok(candidate.to_path_buf());
        }

        let Some(parent) = candidate.parent() else {
            return Err(format!(
                "Could not resolve parent directory for {}.",
                path.display()
            ));
        };
        candidate = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn load_file_preview_defaults_to_highlighted_html() {
        let directory = tempdir().expect("tempdir");
        let file_path = directory.path().join("module.move");
        fs::write(&file_path, "module 0x1::example { fun demo() {} }\n").expect("write source");

        let preview = tauri::async_runtime::block_on(load_file_preview(
            directory.path().to_string_lossy().into_owned(),
            "module.move".to_string(),
            None,
        ))
        .expect("preview");

        let FilePreview::Text {
            highlighted_html,
            source,
            ..
        } = preview
        else {
            panic!("expected text preview");
        };

        assert_eq!(source, "module 0x1::example { fun demo() {} }\n");
        assert!(highlighted_html.contains("<span"));
    }

    #[test]
    fn load_file_preview_can_skip_highlighted_html() {
        let directory = tempdir().expect("tempdir");
        let file_path = directory.path().join("module.move");
        fs::write(&file_path, "module 0x1::example { fun demo() {} }\n").expect("write source");

        let preview = tauri::async_runtime::block_on(load_file_preview(
            directory.path().to_string_lossy().into_owned(),
            "module.move".to_string(),
            Some(false),
        ))
        .expect("preview");

        let FilePreview::Text {
            highlighted_html,
            source,
            ..
        } = preview
        else {
            panic!("expected text preview");
        };

        assert_eq!(source, "module 0x1::example { fun demo() {} }\n");
        assert_eq!(highlighted_html, "");
    }

    #[test]
    fn resolve_write_path_allows_new_nested_file_under_root() {
        let directory = tempdir().expect("tempdir");
        let path = resolve_package_child_write_path(
            &directory.path().to_string_lossy(),
            "tests/security/pgr_001.move",
        )
        .expect("write path");

        assert_eq!(
            path,
            directory
                .path()
                .canonicalize()
                .expect("canonical tempdir")
                .join("tests/security/pgr_001.move")
        );
    }

    #[test]
    fn resolve_write_path_rejects_parent_traversal() {
        let directory = tempdir().expect("tempdir");
        let error =
            resolve_package_child_write_path(&directory.path().to_string_lossy(), "../escape.move")
                .expect_err("parent traversal should fail");

        assert!(error.contains("outside of the package directory"));
    }

    #[test]
    fn build_move_graphs_reports_manifest_only_package() {
        let directory = tempdir().expect("tempdir");
        fs::write(
            directory.path().join("Move.toml"),
            "[package]\nname = \"manifest_only\"\n",
        )
        .expect("manifest");

        let error = match build_move_graphs(
            directory.path().to_string_lossy().into_owned(),
            Some(".".to_string()),
        ) {
            Ok(_) => panic!("manifest-only package should not build source graphs"),
            Err(error) => error,
        };

        assert!(error.contains("no Move source files under sources/"));
        assert!(error.contains(
            "Dependency graph, call graph, type graph, and bytecode views require parseable source modules"
        ));
    }

    #[test]
    fn build_move_graphs_reports_source_files_without_parseable_modules() {
        let directory = tempdir().expect("tempdir");
        fs::write(
            directory.path().join("Move.toml"),
            "[package]\nname = \"generated\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(directory.path().join("sources")).expect("sources");
        fs::write(
            directory.path().join("sources/generated.move"),
            "/*\nmodule generated::generated;\n*/\n",
        )
        .expect("source");

        let error = match build_move_graphs(
            directory.path().to_string_lossy().into_owned(),
            Some(".".to_string()),
        ) {
            Ok(_) => panic!("comment-only package should not build source graphs"),
            Err(error) => error,
        };

        assert!(error.contains("no parseable Move modules"));
        assert!(error.contains("commented out or invalid"));
    }
}
