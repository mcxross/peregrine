use std::path::PathBuf;

use peregrine_plugins::{InstalledPlugin, PluginKind, PluginRegistry};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn list_plugins(
    app: tauri::AppHandle,
    kind: Option<String>,
) -> Result<Vec<InstalledPlugin>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry = plugin_registry(&app)?;

        match kind {
            Some(kind) => registry.list_plugins_by_kind(&PluginKind::new(kind)?),
            None => registry.list_plugins(),
        }
    })
    .await
    .map_err(|error| format!("Could not join plugin list task: {error}"))?
}

#[tauri::command]
pub(crate) async fn remove_plugin(
    app: tauri::AppHandle,
    kind: String,
    plugin_id: String,
) -> Result<Vec<InstalledPlugin>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        plugin_registry(&app)?.remove_plugin(&PluginKind::new(kind)?, &plugin_id)
    })
    .await
    .map_err(|error| format!("Could not join plugin removal task: {error}"))?
}

#[tauri::command]
pub(crate) async fn set_plugin_enabled(
    app: tauri::AppHandle,
    kind: String,
    plugin_id: String,
    enabled: bool,
) -> Result<Vec<InstalledPlugin>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        plugin_registry(&app)?.set_plugin_enabled(&PluginKind::new(kind)?, &plugin_id, enabled)
    })
    .await
    .map_err(|error| format!("Could not join plugin update task: {error}"))?
}

fn plugin_registry(app: &tauri::AppHandle) -> Result<PluginRegistry, String> {
    plugin_registry_root(app).map(PluginRegistry::at_root)
}

fn plugin_registry_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app config directory: {error}"))
}
