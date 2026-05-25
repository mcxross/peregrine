use peregrine_terminal::{TerminalManager, TerminalStartRequest as RuntimeTerminalStartRequest};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

pub(crate) const TERMINAL_OUTPUT_EVENT: &str = "terminal-output";
pub(crate) const TERMINAL_EXIT_EVENT: &str = "terminal-exit";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalStartRequest {
    cwd: String,
    cols: u16,
    rows: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalStartResponse {
    session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalWriteRequest {
    session_id: String,
    data: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalResizeRequest {
    session_id: String,
    cols: u16,
    rows: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalStopRequest {
    session_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputEvent {
    session_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalExitEvent {
    session_id: String,
    code: Option<i32>,
}

#[tauri::command]
pub(crate) async fn terminal_start(
    app: AppHandle,
    terminal_manager: State<'_, TerminalManager>,
    request: TerminalStartRequest,
) -> Result<TerminalStartResponse, String> {
    let output_app = app.clone();
    let exit_app = app;
    let response = terminal_manager.start(
        RuntimeTerminalStartRequest {
            cols: request.cols,
            cwd: request.cwd,
            rows: request.rows,
        },
        move |event| {
            let _ = output_app.emit(
                TERMINAL_OUTPUT_EVENT,
                TerminalOutputEvent {
                    session_id: event.session_id,
                    data: event.data,
                },
            );
        },
        move |event| {
            let _ = exit_app.emit(
                TERMINAL_EXIT_EVENT,
                TerminalExitEvent {
                    session_id: event.session_id,
                    code: event.code,
                },
            );
        },
    )?;

    Ok(TerminalStartResponse {
        session_id: response.session_id,
    })
}

#[tauri::command]
pub(crate) async fn terminal_write(
    terminal_manager: State<'_, TerminalManager>,
    request: TerminalWriteRequest,
) -> Result<(), String> {
    terminal_manager.write(&request.session_id, &request.data)
}

#[tauri::command]
pub(crate) async fn terminal_resize(
    terminal_manager: State<'_, TerminalManager>,
    request: TerminalResizeRequest,
) -> Result<(), String> {
    terminal_manager.resize(&request.session_id, request.cols, request.rows)
}

#[tauri::command]
pub(crate) async fn terminal_stop(
    terminal_manager: State<'_, TerminalManager>,
    request: TerminalStopRequest,
) -> Result<(), String> {
    terminal_manager.stop(&request.session_id)
}
