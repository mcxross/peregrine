use peregrine_sui_adapter::move_analyzer::MoveAnalyzerAdapterSettings;
use std::{fs, path::PathBuf};
use tauri::Manager;

pub(crate) const MOVE_ANALYZER_ADAPTER_SETTINGS_CHANGED_EVENT: &str =
    "move-analyzer-adapter-settings-changed";
const MOVE_ANALYZER_ADAPTER_SETTINGS_FILE: &str = "move-analyzer-adapter-settings.json";

pub(crate) fn load_settings(app: &tauri::AppHandle) -> Result<MoveAnalyzerAdapterSettings, String> {
    let path = settings_path(app)?;

    if !path.is_file() {
        return Ok(MoveAnalyzerAdapterSettings::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Could not read Move Analyzer adapter settings {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "Could not parse Move Analyzer adapter settings {}: {error}",
            path.display()
        )
    })
}

pub(crate) fn store_settings(
    app: &tauri::AppHandle,
    settings: &MoveAnalyzerAdapterSettings,
) -> Result<(), String> {
    let path = settings_path(app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create Move Analyzer adapter settings directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize Move Analyzer adapter settings: {error}"))?;

    fs::write(&path, format!("{contents}\n")).map_err(|error| {
        format!(
            "Could not write Move Analyzer adapter settings {}: {error}",
            path.display()
        )
    })
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app config directory: {error}"))?
        .join(MOVE_ANALYZER_ADAPTER_SETTINGS_FILE))
}
