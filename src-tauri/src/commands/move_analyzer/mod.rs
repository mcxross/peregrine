mod framing;
mod process;
mod settings;

use crate::state::MoveAnalyzerCommandState;
use peregrine_adapters::move_analyzer::{
    MoveAnalyzerAdapter, MoveAnalyzerAdapterEnvironment, MoveAnalyzerAdapterSettings,
    MoveAnalyzerAdapterStatus,
};
use process::MoveAnalyzerServerSession;
use serde_json::Value;
use tauri::Emitter;

#[tauri::command]
pub(crate) async fn check_move_analyzer_adapter(
    app: tauri::AppHandle,
) -> Result<MoveAnalyzerAdapterStatus, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(move_analyzer_adapter(&app)?.status()))
        .await
        .map_err(|error| format!("Could not join Move Analyzer adapter check task: {error}"))?
}

#[tauri::command]
pub(crate) async fn get_move_analyzer_adapter_settings(
    app: tauri::AppHandle,
) -> Result<MoveAnalyzerAdapterSettings, String> {
    tauri::async_runtime::spawn_blocking(move || settings::load_settings(&app))
        .await
        .map_err(|error| format!("Could not join Move Analyzer settings load task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_move_analyzer_adapter_settings(
    app: tauri::AppHandle,
    settings: MoveAnalyzerAdapterSettings,
) -> Result<MoveAnalyzerAdapterSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        settings::store_settings(&app, &settings)?;
        let _ = app.emit(
            settings::MOVE_ANALYZER_ADAPTER_SETTINGS_CHANGED_EVENT,
            &settings,
        );

        Ok(settings)
    })
    .await
    .map_err(|error| format!("Could not join Move Analyzer settings save task: {error}"))?
}

#[tauri::command]
pub(crate) async fn start_move_analyzer_server(
    app: tauri::AppHandle,
    state: tauri::State<'_, MoveAnalyzerCommandState>,
    root_path: String,
) -> Result<MoveAnalyzerServerSession, String> {
    let command = move_analyzer_adapter(&app)?
        .server_command()
        .map_err(|error| error.to_string())?;

    process::start_server(app, state, &root_path, command)
}

#[tauri::command]
pub(crate) async fn send_move_analyzer_message(
    state: tauri::State<'_, MoveAnalyzerCommandState>,
    session_id: String,
    message: Value,
) -> Result<(), String> {
    process::send_message(&state, &session_id, message)
}

#[tauri::command]
pub(crate) async fn stop_move_analyzer_server(
    state: tauri::State<'_, MoveAnalyzerCommandState>,
    session_id: String,
) -> Result<(), String> {
    process::stop_session_by_id(&state, &session_id)
}

fn move_analyzer_adapter(app: &tauri::AppHandle) -> Result<MoveAnalyzerAdapter, String> {
    Ok(MoveAnalyzerAdapter::new(
        settings::load_settings(app)?,
        MoveAnalyzerAdapterEnvironment::new(),
    ))
}
