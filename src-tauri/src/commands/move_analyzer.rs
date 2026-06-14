use crate::state::MoveAnalyzerMcpState;
use peregrine_mcp_client::{
    McpClientHandle, McpClientOptions, McpClientRuntime, McpExecutionOrigin,
};
use peregrine_sui_move_analyzer_mcp_protocol::{
    CompletionArgs, DocumentArgs, MoveAnalyzerAdapterSettings, MoveAnalyzerAdapterSource,
    PositionArgs, RenameArgs, SERVER_NAME, tool_name,
};
use peregrine_utils_home_dir::find_peregrine_home;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::Emitter;
use toml_edit::{DocumentMut, Item, Table, value};

const CONFIG_CHANGED_EVENT: &str = "sui-move-analyzer-config-changed";

#[tauri::command]
pub(crate) async fn check_sui_move_analyzer_adapter(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
) -> Result<Value, String> {
    let root = std::env::current_dir()
        .map_err(|error| format!("Could not resolve the current workspace: {error}"))?;
    let root = root
        .to_str()
        .ok_or_else(|| "The current workspace path is not valid UTF-8".to_string())?;
    call(&state, root, tool_name::STATUS, json!({})).await
}

#[tauri::command]
pub(crate) async fn get_sui_move_analyzer_settings() -> Result<MoveAnalyzerAdapterSettings, String>
{
    tauri::async_runtime::spawn_blocking(load_settings)
        .await
        .map_err(|error| format!("Could not join Move Analyzer settings load task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_sui_move_analyzer_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    settings: MoveAnalyzerAdapterSettings,
) -> Result<MoveAnalyzerAdapterSettings, String> {
    let saved = settings.clone();
    tauri::async_runtime::spawn_blocking(move || store_settings(&saved))
        .await
        .map_err(|error| format!("Could not join Move Analyzer settings save task: {error}"))??;
    clear_runtimes(&state);
    let _ = app.emit(CONFIG_CHANGED_EVENT, &settings);
    Ok(settings)
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_status(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
) -> Result<Value, String> {
    call(&state, &root_path, tool_name::STATUS, json!({})).await
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_diagnostics(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
    document: DocumentArgs,
) -> Result<Value, String> {
    call(
        &state,
        &root_path,
        tool_name::DIAGNOSTICS,
        serde_json::to_value(document).map_err(|error| error.to_string())?,
    )
    .await
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_completion(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
    request: CompletionArgs,
) -> Result<Value, String> {
    call(
        &state,
        &root_path,
        tool_name::COMPLETION,
        serde_json::to_value(request).map_err(|error| error.to_string())?,
    )
    .await
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_hover(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
    request: PositionArgs,
) -> Result<Value, String> {
    call_typed(&state, &root_path, tool_name::HOVER, request).await
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_definition(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
    request: PositionArgs,
) -> Result<Value, String> {
    call_typed(&state, &root_path, tool_name::DEFINITION, request).await
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_references(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
    request: PositionArgs,
) -> Result<Value, String> {
    call_typed(&state, &root_path, tool_name::REFERENCES, request).await
}

#[tauri::command]
pub(crate) async fn sui_move_analyzer_rename(
    state: tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: String,
    request: RenameArgs,
) -> Result<Value, String> {
    call_typed(&state, &root_path, tool_name::RENAME, request).await
}

async fn call_typed(
    state: &tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: &str,
    tool: &str,
    request: impl serde::Serialize,
) -> Result<Value, String> {
    call(
        state,
        root_path,
        tool,
        serde_json::to_value(request).map_err(|error| error.to_string())?,
    )
    .await
}

async fn call(
    state: &tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: &str,
    tool: &str,
    arguments: Value,
) -> Result<Value, String> {
    runtime_handle(state, root_path)?
        .call(SERVER_NAME, tool, arguments)
        .await
}

fn runtime_handle(
    state: &tauri::State<'_, MoveAnalyzerMcpState>,
    root_path: &str,
) -> Result<McpClientHandle, String> {
    let root = canonical_root(root_path)?;
    let mut runtimes = state
        .runtimes
        .lock()
        .map_err(|_| "Move Analyzer MCP runtime state is poisoned".to_string())?;
    if let Some(runtime) = runtimes.get(&root) {
        return Ok(runtime.handle());
    }
    let peregrine_home = find_peregrine_home()
        .map_err(|error| format!("failed to resolve PEREGRINE_HOME: {error}"))?;
    let runtime = Arc::new(McpClientRuntime::load(McpClientOptions::new(
        root.clone(),
        peregrine_home.into_path_buf(),
        McpExecutionOrigin::Workbench,
    ))?);
    let handle = runtime.handle();
    runtimes.insert(root, runtime);
    Ok(handle)
}

fn clear_runtimes(state: &tauri::State<'_, MoveAnalyzerMcpState>) {
    if let Ok(mut runtimes) = state.runtimes.lock() {
        runtimes.clear();
    }
}

fn canonical_root(root_path: &str) -> Result<PathBuf, String> {
    let root = Path::new(root_path)
        .canonicalize()
        .map_err(|error| format!("Could not resolve Move Analyzer root {root_path}: {error}"))?;
    if !root.is_dir() {
        return Err("Move Analyzer root path is not a directory".to_string());
    }
    Ok(root)
}

fn load_settings() -> Result<MoveAnalyzerAdapterSettings, String> {
    let path = config_path()?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MoveAnalyzerAdapterSettings::default());
        }
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };
    let config: peregrine_config::config_toml::ConfigToml = toml::from_str(&raw)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))?;
    let Some(adapter) = config
        .tools
        .and_then(|tools| tools.sui_move_analyzer)
        .and_then(|tools| tools.adapter)
    else {
        return Ok(MoveAnalyzerAdapterSettings::default());
    };
    Ok(MoveAnalyzerAdapterSettings {
        source: match adapter.source {
            Some(peregrine_config::config_toml::SuiAdapterSourceToml::System) => {
                MoveAnalyzerAdapterSource::System
            }
            Some(peregrine_config::config_toml::SuiAdapterSourceToml::Bundled) | None => {
                MoveAnalyzerAdapterSource::Bundled
            }
        },
        binary_path: adapter.binary_path,
    })
}

fn store_settings(settings: &MoveAnalyzerAdapterSettings) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };
    let mut document = if raw.trim().is_empty() {
        DocumentMut::new()
    } else {
        raw.parse::<DocumentMut>()
            .map_err(|error| format!("Could not parse {}: {error}", path.display()))?
    };
    ensure_table(document.as_table_mut(), "tools");
    let tools = document["tools"]
        .as_table_mut()
        .ok_or_else(|| "`tools` must be a table".to_string())?;
    ensure_table(tools, "sui_move_analyzer");
    let analyzer = tools["sui_move_analyzer"]
        .as_table_mut()
        .ok_or_else(|| "`tools.sui_move_analyzer` must be a table".to_string())?;
    ensure_table(analyzer, "adapter");
    let adapter = analyzer["adapter"]
        .as_table_mut()
        .ok_or_else(|| "`tools.sui_move_analyzer.adapter` must be a table".to_string())?;
    adapter["source"] = value(settings.source.as_str());
    match settings
        .binary_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        Some(binary_path) => adapter["binary_path"] = value(binary_path),
        None => {
            adapter.remove("binary_path");
        }
    }
    fs::write(&path, document.to_string())
        .map_err(|error| format!("Could not write {}: {error}", path.display()))
}

fn ensure_table(parent: &mut Table, key: &str) {
    if !parent.contains_key(key) {
        parent.insert(key, Item::Table(Table::new()));
    }
}

fn config_path() -> Result<PathBuf, String> {
    find_peregrine_home()
        .map(|home| {
            home.into_path_buf()
                .join(peregrine_config::CONFIG_TOML_FILE)
        })
        .map_err(|error| format!("failed to resolve PEREGRINE_HOME: {error}"))
}
