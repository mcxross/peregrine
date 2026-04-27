mod file_preview;
mod move_project;

use base64::{engine::general_purpose, Engine};
use file_preview::{build_file_preview, FilePreview};
use move_project::{discover_move_project, MovePackage, PackageDependencyGraph};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{
    menu::{AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu},
    Emitter,
};

const OPEN_SETTINGS_MENU_ID: &str = "open-settings";
const OPEN_SETTINGS_EVENT: &str = "open-settings";

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Peregrine!", name)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageTree {
    root_path: String,
    root_name: String,
    paths: Vec<String>,
    move_packages: Vec<MovePackage>,
    dependency_graph: PackageDependencyGraph,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandOutput {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuiCliStatus {
    installed: bool,
    version: Option<String>,
    install_hint: Option<String>,
}

#[tauri::command]
async fn load_package_tree(root_path: String) -> Result<PackageTree, String> {
    tauri::async_runtime::spawn_blocking(move || build_package_tree(root_path))
        .await
        .map_err(|error| format!("Could not join package tree task: {error}"))?
}

#[tauri::command]
async fn load_file_preview(
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
async fn save_text_file(
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
async fn save_graph_png(path: String, png_data_url: String) -> Result<(), String> {
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
async fn build_move_package(
    root_path: String,
    package_path: String,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let package_root = resolve_package_child_path(&root_path, &package_path)?;

        if !package_root.is_dir() {
            return Err("Selected package path is not a directory.".to_string());
        }

        if !package_root.join("Move.toml").is_file() {
            return Err("Selected package does not contain a Move.toml file.".to_string());
        }

        let output = Command::new("sui")
            .args(["move", "build"])
            .current_dir(&package_root)
            .output()
            .map_err(|error| {
                format!(
                    "Could not execute `sui move build` in {}: {error}",
                    package_root.display()
                )
            })?;

        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    })
    .await
    .map_err(|error| format!("Could not join package build task: {error}"))?
}

#[tauri::command]
async fn check_sui_cli() -> Result<SuiCliStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let output = match Command::new("sui").arg("--version").output() {
            Ok(output) => output,
            Err(_) => {
                return Ok(SuiCliStatus {
                    installed: false,
                    version: None,
                    install_hint: Some(
                        "Install the Sui CLI and make sure `sui` is on PATH.".to_string(),
                    ),
                });
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let version_source = stdout
            .lines()
            .chain(stderr.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");

        Ok(SuiCliStatus {
            installed: output.status.success(),
            version: parse_sui_version(version_source),
            install_hint: if output.status.success() {
                None
            } else {
                Some("Install the Sui CLI and make sure `sui` is on PATH.".to_string())
            },
        })
    })
    .await
    .map_err(|error| format!("Could not join Sui CLI check task: {error}"))?
}

fn parse_sui_version(source: &str) -> Option<String> {
    source
        .split_whitespace()
        .find(|token| {
            token
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
        .map(|version| version.trim_start_matches('v').to_string())
}

fn build_package_tree(root_path: String) -> Result<PackageTree, String> {
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

    let (move_packages, dependency_graph) = discover_move_project(&root);

    Ok(PackageTree {
        root_path: root.to_string_lossy().into_owned(),
        root_name,
        paths,
        move_packages,
        dependency_graph,
    })
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
            paths.push(format!("{relative_path}/"));
            collect_paths(root, &path, paths)?;
        } else {
            paths.push(relative_path);
        }
    }

    Ok(())
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

fn resolve_package_child_path(root_path: &str, relative_path: &str) -> Result<PathBuf, String> {
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

fn app_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let package_info = app.package_info();
    let config = app.config();
    let about_metadata = AboutMetadata {
        name: Some(package_info.name.clone()),
        version: Some(package_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let settings = MenuItemBuilder::with_id(OPEN_SETTINGS_MENU_ID, "Settings...")
        .accelerator("Cmd+,")
        .build(app)?;

    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let help_menu = Submenu::with_items(app, "Help", true, &[])?;

    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                package_info.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about_metadata))?,
                    &settings,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "File",
                true,
                &[&PredefinedMenuItem::close_window(app, None)?],
            )?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app, None)?],
            )?,
            &window_menu,
            &help_menu,
        ],
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .menu(app_menu)
        .on_menu_event(|app, event| {
            if event.id().as_ref() == OPEN_SETTINGS_MENU_ID {
                let _ = app.emit(OPEN_SETTINGS_EVENT, ());
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            load_package_tree,
            load_file_preview,
            save_text_file,
            save_graph_png,
            build_move_package,
            check_sui_cli
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
