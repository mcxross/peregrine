mod file_preview;

use base64::{engine::general_purpose, Engine};
use file_preview::{build_file_preview, FilePreview};
use peregrine_bytecode_view::{load_package_bytecode, MoveBytecodePackageView};
use peregrine_static_analysis::{
    discover_move_project_fast, discover_move_project_shallow, discover_project_graphs,
    discover_project_graphs_for_package, discover_state_access_graph_for_function, AnalysisConfig,
    AnalysisReport, Analyzer, MoveCallGraph, MovePackage, MoveProjectGraphs, MoveStateAccessGraph,
    MoveTypeGraph, PackageDependencyGraph,
};
use peregrine_sui_adapter::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiAdapterStatus, SuiExecutionTarget,
    SuiPackageCommand,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
use tauri::{
    menu::{AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu},
    Emitter, Manager,
};

const OPEN_SETTINGS_MENU_ID: &str = "open-settings";
const OPEN_SETTINGS_EVENT: &str = "open-settings";
const CLOSE_PROJECT_MENU_ID: &str = "close-project";
const CLOSE_PROJECT_EVENT: &str = "close-project";
const COMMAND_OUTPUT_EVENT: &str = "command-output";
const BUNDLED_SUI_HELPER_ARG: &str = "--peregrine-bundled-sui";
const MOVY_FUZZ_HELPER_ARG: &str = "--peregrine-movy-fuzz";
const PROJECT_METADATA_DIRECTORY: &str = ".peregrine";
const PROJECT_METADATA_FILE: &str = "metadata.json";
const SUI_ADAPTER_SETTINGS_CHANGED_EVENT: &str = "sui-adapter-settings-changed";
const SUI_ADAPTER_SETTINGS_FILE: &str = "sui-adapter-settings.json";
const OLLAMA_CHAT_STREAM_EVENT: &str = "ollama-chat-stream";
const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const OLLAMA_FALLBACK_BASE_URL: &str = "http://localhost:11434";
const OLLAMA_KEEP_ALIVE: &str = "30m";

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Peregrine!", name)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageTree {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandOutput {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CommandOutputChunk {
    stream_id: String,
    stream: &'static str,
    chunk: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProjectMetadata {
    #[serde(default = "default_project_metadata_version")]
    version: u32,
    #[serde(default)]
    builds: HashMap<String, ProjectBuildMetadata>,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            version: default_project_metadata_version(),
            builds: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectBuildMetadata {
    last_successful_build_at: Option<u64>,
}

fn default_project_metadata_version() -> u32 {
    1
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatApiResponse {
    message: Option<OllamaChatMessage>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatPayload {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    think: bool,
    #[serde(rename = "keep_alive")]
    keep_alive: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OllamaChatStreamEvent {
    stream_id: String,
    kind: &'static str,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatStreamResponse {
    done: Option<bool>,
    message: Option<OllamaChatMessage>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatResponse {
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaGenerateApiResponse {
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaPreloadPayload {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(rename = "keep_alive")]
    keep_alive: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaPreloadResponse {
    model: String,
    keep_alive: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaTagsApiResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaModel {
    name: String,
}

#[derive(Clone)]
struct CommandOutputStream {
    app: tauri::AppHandle,
    stream_id: String,
}

struct CommandReaderChunk {
    stream: &'static str,
    bytes: Vec<u8>,
}

struct OllamaStreamError {
    had_content: bool,
    message: String,
}

#[tauri::command]
async fn load_package_tree(root_path: String) -> Result<PackageTree, String> {
    tauri::async_runtime::spawn_blocking(move || {
        build_package_tree(root_path, PackageTreeMode::Shallow)
    })
    .await
    .map_err(|error| format!("Could not join package tree task: {error}"))?
}

#[tauri::command]
async fn load_package_tree_details(root_path: String) -> Result<PackageTree, String> {
    tauri::async_runtime::spawn_blocking(move || {
        build_package_tree(root_path, PackageTreeMode::Detailed)
    })
    .await
    .map_err(|error| format!("Could not join package detail task: {error}"))?
}

#[tauri::command]
async fn load_move_graphs(
    root_path: String,
    package_path: Option<String>,
) -> Result<MoveProjectGraphs, String> {
    tauri::async_runtime::spawn_blocking(move || build_move_graphs(root_path, package_path))
        .await
        .map_err(|error| format!("Could not join Move graph task: {error}"))?
}

#[tauri::command]
async fn load_move_state_access_graph(
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
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sui = sui_adapter(&app)?;
        run_package_command(
            &root_path,
            &package_path,
            sui.package_command("move-build")
                .map_err(|error| error.to_string())?,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join package build task: {error}"))?
}

#[tauri::command]
async fn run_security_command(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    command_kind: String,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sui = sui_adapter(&app)?;
        let command = sui
            .package_command(&command_kind)
            .map_err(|error| error.to_string())?;

        run_package_command(
            &root_path,
            &package_path,
            command,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join security command task: {error}"))?
}

#[tauri::command]
async fn run_movy_fuzz(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_movy_fuzz_worker(
            &root_path,
            &package_path,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join Movy fuzz task: {error}"))?
}

fn run_movy_fuzz_worker(
    root_path: &str,
    package_path: &str,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let header = "Deploying package into Movy's local Sui executor and starting Movy fuzzing...\n";
    emit_command_output_chunk(stream.as_ref(), "stdout", header);

    let executable = std::env::current_exe()
        .map_err(|error| format!("Could not resolve Peregrine executable: {error}"))?;
    let mut process = Command::new(executable);
    process
        .arg(MOVY_FUZZ_HELPER_ARG)
        .arg(root_path)
        .arg(package_path);

    let mut output = run_configured_command(configure_plain_command_output(&mut process), stream)?;
    output.stdout = format!("{header}{}", output.stdout);

    Ok(output)
}

#[tauri::command]
async fn run_security_script(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    script_path: String,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_package_script(
            &root_path,
            &package_path,
            &script_path,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join security script task: {error}"))?
}

#[tauri::command]
async fn analyze_move_package(
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

        Ok(Analyzer::new().analyze_package(package_root, config))
    })
    .await
    .map_err(|error| format!("Could not join Move analysis task: {error}"))?
}

#[tauri::command]
async fn load_move_bytecode_view(
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

#[tauri::command]
async fn check_sui_adapter(app: tauri::AppHandle) -> Result<SuiAdapterStatus, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(sui_adapter(&app)?.status()))
        .await
        .map_err(|error| format!("Could not join Sui adapter check task: {error}"))?
}

#[tauri::command]
async fn get_sui_adapter_settings(app: tauri::AppHandle) -> Result<SuiAdapterSettings, String> {
    tauri::async_runtime::spawn_blocking(move || load_sui_adapter_settings(&app))
        .await
        .map_err(|error| format!("Could not join Sui adapter settings load task: {error}"))?
}

#[tauri::command]
async fn save_sui_adapter_settings(
    app: tauri::AppHandle,
    settings: SuiAdapterSettings,
) -> Result<SuiAdapterSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        store_sui_adapter_settings(&app, &settings)?;
        let _ = app.emit(SUI_ADAPTER_SETTINGS_CHANGED_EVENT, &settings);

        Ok(settings)
    })
    .await
    .map_err(|error| format!("Could not join Sui adapter settings save task: {error}"))?
}

#[tauri::command]
async fn load_project_metadata(root_path: String) -> Result<ProjectMetadata, String> {
    tauri::async_runtime::spawn_blocking(move || read_project_metadata(&root_path))
        .await
        .map_err(|error| format!("Could not join project metadata load task: {error}"))?
}

#[tauri::command]
async fn save_project_metadata(
    root_path: String,
    metadata: ProjectMetadata,
) -> Result<ProjectMetadata, String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_project_metadata(&root_path, &metadata)?;
        Ok(metadata)
    })
    .await
    .map_err(|error| format!("Could not join project metadata save task: {error}"))?
}

#[tauri::command]
async fn list_ollama_models() -> Result<Vec<OllamaModel>, String> {
    let client = ollama_client()?;
    let body = send_ollama_request(&client, "/api/tags", "model list", |client, url| {
        client.get(url).header("Accept", "application/json")
    })
    .await?;
    let tags: OllamaTagsApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Ollama model list: {error}"))?;

    Ok(tags.models)
}

#[tauri::command]
async fn chat_with_ollama(
    model: String,
    messages: Vec<OllamaChatMessage>,
) -> Result<OllamaChatResponse, String> {
    let payload = OllamaChatPayload {
        model: model.trim().to_string(),
        messages,
        stream: false,
        think: false,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    };
    let request_body = serde_json::to_string(&payload)
        .map_err(|error| format!("Could not serialize Ollama request: {error}"))?;
    let client = ollama_client()?;
    let body = send_ollama_request(&client, "/api/chat", "chat", |client, url| {
        client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(request_body.clone())
    })
    .await?;
    let chat: OllamaChatApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Ollama response: {error}"))?;

    if let Some(error) = chat.error {
        return Err(error);
    }

    Ok(OllamaChatResponse {
        content: chat
            .message
            .map(|message| message.content)
            .unwrap_or_default(),
    })
}

#[tauri::command]
async fn preload_ollama_model(model: String) -> Result<OllamaPreloadResponse, String> {
    let model = model.trim().to_string();

    if model.is_empty() {
        return Err("Select an Ollama model before preloading.".to_string());
    }

    let payload = OllamaPreloadPayload {
        model: model.clone(),
        prompt: String::new(),
        stream: false,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    };
    let request_body = serde_json::to_string(&payload)
        .map_err(|error| format!("Could not serialize Ollama preload request: {error}"))?;
    let client = ollama_client()?;

    eprintln!("[peregrine:ollama] Preloading model={model} keep_alive={OLLAMA_KEEP_ALIVE}");

    let body = send_ollama_request(&client, "/api/generate", "preload", |client, url| {
        client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(request_body.clone())
    })
    .await?;
    let response: OllamaGenerateApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Ollama preload response: {error}"))?;

    if let Some(error) = response.error {
        return Err(error);
    }

    eprintln!("[peregrine:ollama] Preloaded model={model}");

    Ok(OllamaPreloadResponse {
        model,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    })
}

#[tauri::command]
async fn stream_chat_with_ollama(
    app: tauri::AppHandle,
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream_id: String,
) -> Result<OllamaChatResponse, String> {
    let payload = OllamaChatPayload {
        model: model.trim().to_string(),
        messages,
        stream: true,
        think: false,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    };
    let request_body = serde_json::to_string(&payload)
        .map_err(|error| format!("Could not serialize Ollama stream request: {error}"))?;
    let client = ollama_client()?;
    let base_urls = [OLLAMA_BASE_URL, OLLAMA_FALLBACK_BASE_URL];
    let mut errors = Vec::new();

    emit_ollama_log(
        &app,
        &stream_id,
        format!(
            "Preparing Ollama stream request: endpoint=/api/chat model={} messages={} bytes={}",
            payload.model,
            payload.messages.len(),
            request_body.len()
        ),
    );

    for base_url in base_urls {
        match stream_single_ollama_chat(&client, base_url, &request_body, &app, &stream_id).await {
            Ok(content) => {
                emit_ollama_event(&app, &stream_id, "done", "");
                return Ok(OllamaChatResponse { content });
            }
            Err(error) => {
                emit_ollama_log(&app, &stream_id, error.message.clone());

                if error.had_content {
                    emit_ollama_event(&app, &stream_id, "error", &error.message);
                    return Err(error.message);
                }

                errors.push(error.message);
            }
        }
    }

    let error = format!(
        "Could not stream from local Ollama. Tried {}. {}",
        base_urls.join(", "),
        errors.join(" ")
    );
    emit_ollama_event(&app, &stream_id, "error", &error);
    Err(error)
}

fn sui_adapter(app: &tauri::AppHandle) -> Result<SuiAdapter, String> {
    Ok(SuiAdapter::new(
        load_sui_adapter_settings(app)?,
        SuiAdapterEnvironment::new(),
    ))
}

fn load_sui_adapter_settings(app: &tauri::AppHandle) -> Result<SuiAdapterSettings, String> {
    let path = sui_adapter_settings_path(app)?;

    if !path.is_file() {
        return Ok(SuiAdapterSettings::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Could not read Sui adapter settings {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "Could not parse Sui adapter settings {}: {error}",
            path.display()
        )
    })
}

fn store_sui_adapter_settings(
    app: &tauri::AppHandle,
    settings: &SuiAdapterSettings,
) -> Result<(), String> {
    let path = sui_adapter_settings_path(app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create Sui adapter settings directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize Sui adapter settings: {error}"))?;

    fs::write(&path, format!("{contents}\n")).map_err(|error| {
        format!(
            "Could not write Sui adapter settings {}: {error}",
            path.display()
        )
    })
}

fn sui_adapter_settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app config directory: {error}"))?
        .join(SUI_ADAPTER_SETTINGS_FILE))
}

fn read_project_metadata(root_path: &str) -> Result<ProjectMetadata, String> {
    let path = project_metadata_path(root_path)?;

    if !path.exists() {
        return Ok(ProjectMetadata::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Could not read project metadata {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "Could not parse project metadata {}: {error}",
            path.display()
        )
    })
}

fn write_project_metadata(root_path: &str, metadata: &ProjectMetadata) -> Result<(), String> {
    let path = project_metadata_path(root_path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create project metadata directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let contents = serde_json::to_string_pretty(metadata)
        .map_err(|error| format!("Could not serialize project metadata: {error}"))?;

    fs::write(&path, format!("{contents}\n")).map_err(|error| {
        format!(
            "Could not write project metadata {}: {error}",
            path.display()
        )
    })
}

fn project_metadata_path(root_path: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;

    Ok(root
        .join(PROJECT_METADATA_DIRECTORY)
        .join(PROJECT_METADATA_FILE))
}

fn ollama_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .http1_only()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(240))
        .build()
        .map_err(|error| format!("Could not create Ollama HTTP client: {error}"))
}

async fn send_ollama_request<F>(
    client: &reqwest::Client,
    path: &str,
    action: &str,
    build_request: F,
) -> Result<String, String>
where
    F: Fn(&reqwest::Client, String) -> reqwest::RequestBuilder,
{
    let base_urls = [OLLAMA_BASE_URL, OLLAMA_FALLBACK_BASE_URL];
    let mut errors = Vec::new();

    for base_url in base_urls {
        let url = format!("{base_url}{path}");
        match send_single_ollama_request(build_request(client, url), action, base_url).await {
            Ok(body) => return Ok(body),
            Err(error) => errors.push(error),
        }
    }

    Err(format!(
        "Could not reach local Ollama while requesting {action}. Tried {}. {}",
        base_urls.join(", "),
        errors.join(" ")
    ))
}

async fn send_single_ollama_request(
    request: reqwest::RequestBuilder,
    action: &str,
    base_url: &str,
) -> Result<String, String> {
    let response = request.send().await.map_err(|error| {
        format!(
            "Request to {base_url} failed: {}",
            describe_reqwest_error(&error)
        )
    })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        format!(
            "Could not read Ollama {action} response from {base_url}: {}",
            describe_reqwest_error(&error)
        )
    })?;

    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| body.trim().to_string());

        return Err(if message.is_empty() {
            format!("Ollama at {base_url} returned HTTP {status} while requesting {action}.")
        } else {
            format!(
                "Ollama at {base_url} returned HTTP {status} while requesting {action}: {message}"
            )
        });
    }

    Ok(body)
}

async fn stream_single_ollama_chat(
    client: &reqwest::Client,
    base_url: &str,
    request_body: &str,
    app: &tauri::AppHandle,
    stream_id: &str,
) -> Result<String, OllamaStreamError> {
    let url = format!("{base_url}/api/chat");
    emit_ollama_log(app, stream_id, format!("Opening Ollama stream: {url}"));

    let mut response = client
        .post(url)
        .header("Accept", "application/x-ndjson")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .send()
        .await
        .map_err(|error| OllamaStreamError {
            had_content: false,
            message: format!(
                "Stream request to {base_url} failed: {}",
                describe_reqwest_error(&error)
            ),
        })?;
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.map_err(|error| OllamaStreamError {
            had_content: false,
            message: format!(
                "Could not read Ollama stream error response from {base_url}: {}",
                describe_reqwest_error(&error)
            ),
        })?;
        let message = parse_ollama_error_body(&body);

        return Err(OllamaStreamError {
            had_content: false,
            message: if message.is_empty() {
                format!("Ollama at {base_url} returned HTTP {status} for stream request.")
            } else {
                format!("Ollama at {base_url} returned HTTP {status} for stream request: {message}")
            },
        });
    }

    let mut pending = Vec::<u8>::new();
    let mut content = String::new();

    while let Some(chunk) = response.chunk().await.map_err(|error| OllamaStreamError {
        had_content: !content.is_empty(),
        message: format!(
            "Could not read Ollama stream chunk from {base_url}: {}",
            describe_reqwest_error(&error)
        ),
    })? {
        pending.extend_from_slice(&chunk);

        while let Some(line_end) = pending.iter().position(|byte| *byte == b'\n') {
            let line = pending.drain(..=line_end).collect::<Vec<_>>();
            process_ollama_stream_line(&line, &mut content, app, stream_id).map_err(|message| {
                OllamaStreamError {
                    had_content: !content.is_empty(),
                    message,
                }
            })?;
        }
    }

    if !pending.is_empty() {
        process_ollama_stream_line(&pending, &mut content, app, stream_id).map_err(|message| {
            OllamaStreamError {
                had_content: !content.is_empty(),
                message,
            }
        })?;
    }

    emit_ollama_log(
        app,
        stream_id,
        format!(
            "Ollama stream completed from {base_url}; response_bytes={}",
            content.len()
        ),
    );

    Ok(content)
}

fn process_ollama_stream_line(
    line: &[u8],
    content: &mut String,
    app: &tauri::AppHandle,
    stream_id: &str,
) -> Result<(), String> {
    let line = String::from_utf8_lossy(line).trim().to_string();

    if line.is_empty() {
        return Ok(());
    }

    let response: OllamaChatStreamResponse = serde_json::from_str(&line)
        .map_err(|error| format!("Could not parse Ollama stream line: {error}; line={line}"))?;

    if let Some(error) = response.error {
        return Err(error);
    }

    if let Some(message) = response.message {
        if !message.content.is_empty() {
            content.push_str(&message.content);
            emit_ollama_event(app, stream_id, "chunk", &message.content);
        }
    }

    if response.done.unwrap_or(false) {
        emit_ollama_log(app, stream_id, "Ollama stream reported done.".to_string());
    }

    Ok(())
}

fn parse_ollama_error_body(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.trim().to_string())
}

fn describe_reqwest_error(error: &reqwest::Error) -> String {
    let mut details = vec![error.to_string()];
    let mut source = std::error::Error::source(error);

    while let Some(next_source) = source {
        details.push(next_source.to_string());
        source = next_source.source();
    }

    details.join(": ")
}

fn emit_ollama_log(app: &tauri::AppHandle, stream_id: &str, message: String) {
    eprintln!("[peregrine:ollama] {message}");
    emit_ollama_event(app, stream_id, "debug", &message);
}

fn emit_ollama_event(app: &tauri::AppHandle, stream_id: &str, kind: &'static str, content: &str) {
    let _ = app.emit(
        OLLAMA_CHAT_STREAM_EVENT,
        OllamaChatStreamEvent {
            stream_id: stream_id.to_string(),
            kind,
            content: content.to_string(),
        },
    );
}

fn run_package_command(
    root_path: &str,
    package_path: &str,
    command: SuiPackageCommand,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let package_root = resolve_package_child_path(root_path, package_path)?;

    if !package_root.is_dir() {
        return Err("Selected package path is not a directory.".to_string());
    }

    if !package_root.join("Move.toml").is_file() {
        return Err("Selected package does not contain a Move.toml file.".to_string());
    }

    let output = match &command.execution {
        SuiExecutionTarget::Bundled => run_bundled_package_command(&command, &package_root, stream)
            .map_err(|error| {
                format!(
                    "Could not execute bundled `{}` for {}: {error}",
                    command.display,
                    package_root.display()
                )
            })?,
        SuiExecutionTarget::System { executable } => {
            let mut process = Command::new(executable);
            run_configured_command(
                configure_plain_command_output(
                    process.args(&command.args).current_dir(&package_root),
                ),
                stream,
            )
            .map_err(|error| {
                format!(
                    "Could not execute `{}` in {}: {error}",
                    command.display,
                    package_root.display()
                )
            })?
        }
    };
    cleanup_temp_pubfile(command.temp_pubfile_path.as_deref());

    Ok(output)
}

fn run_bundled_package_command(
    command: &SuiPackageCommand,
    package_root: &Path,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let header = format!(
        "Running bundled Sui crate from the linked app dependency: {}\n",
        command.display
    );

    emit_command_output_chunk(stream.as_ref(), "stdout", &header);

    let executable = std::env::current_exe()
        .map_err(|error| format!("Could not resolve Peregrine executable: {error}"))?;
    let mut process = Command::new(executable);
    process
        .arg(BUNDLED_SUI_HELPER_ARG)
        .args(command.bundled_args_for_package(package_root));

    let mut output = run_configured_command(&mut process, stream)?;
    output.stdout = format!("{header}{}", output.stdout);

    Ok(output)
}

fn emit_command_output_chunk(
    stream: Option<&CommandOutputStream>,
    stream_name: &'static str,
    chunk: impl AsRef<str>,
) {
    let chunk = chunk.as_ref();

    if chunk.is_empty() {
        return;
    }

    if let Some(stream) = stream {
        let _ = stream.app.emit(
            COMMAND_OUTPUT_EVENT,
            CommandOutputChunk {
                stream_id: stream.stream_id.clone(),
                stream: stream_name,
                chunk: chunk.to_string(),
            },
        );
    }
}

fn configure_plain_command_output(command: &mut Command) -> &mut Command {
    command
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("TERM", "dumb")
}

fn command_output_stream(
    app: tauri::AppHandle,
    stream_id: Option<String>,
) -> Option<CommandOutputStream> {
    let stream_id = stream_id?.trim().to_string();

    if stream_id.is_empty() {
        return None;
    }

    Some(CommandOutputStream { app, stream_id })
}

fn run_configured_command(
    command: &mut Command,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start process: {error}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel::<CommandReaderChunk>();

    if let Some(stdout) = stdout {
        spawn_output_reader(stdout, "stdout", sender.clone());
    }

    if let Some(stderr) = stderr {
        spawn_output_reader(stderr, "stderr", sender.clone());
    }

    drop(sender);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    for chunk in receiver {
        match chunk.stream {
            "stdout" => stdout.extend_from_slice(&chunk.bytes),
            "stderr" => stderr.extend_from_slice(&chunk.bytes),
            _ => {}
        }

        if let Some(stream) = stream.as_ref() {
            let _ = stream.app.emit(
                COMMAND_OUTPUT_EVENT,
                CommandOutputChunk {
                    stream_id: stream.stream_id.clone(),
                    stream: chunk.stream,
                    chunk: String::from_utf8_lossy(&chunk.bytes).into_owned(),
                },
            );
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("Could not wait for process: {error}"))?;

    Ok(CommandOutput {
        status: status.code(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn spawn_output_reader<R>(
    mut reader: R,
    stream: &'static str,
    sender: mpsc::Sender<CommandReaderChunk>,
) where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if sender
                        .send(CommandReaderChunk {
                            stream,
                            bytes: buffer[..size].to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn cleanup_temp_pubfile(path: Option<&Path>) {
    if let Some(path) = path {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
}

fn run_package_script(
    root_path: &str,
    package_path: &str,
    script_path: &str,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let package_root = resolve_package_child_path(root_path, package_path)?;
    let script_path = script_path.trim();

    if script_path.is_empty() {
        return Err("Bash script path cannot be empty.".to_string());
    }

    if script_path.len() > 1_024 {
        return Err("Bash script path is too long.".to_string());
    }

    if script_path.contains('\0') {
        return Err("Bash script path contains an invalid null byte.".to_string());
    }

    if !package_root.is_dir() {
        return Err("Selected package path is not a directory.".to_string());
    }

    if !package_root.join("Move.toml").is_file() {
        return Err("Selected package does not contain a Move.toml file.".to_string());
    }

    let script_path = resolve_package_script_path(&package_root, script_path)?;

    let mut process = Command::new("bash");
    run_configured_command(
        configure_plain_command_output(process.arg(&script_path).current_dir(&package_root)),
        stream,
    )
    .map_err(|error| {
        format!(
            "Could not execute bash script {} in {}: {error}",
            script_path.display(),
            package_root.display()
        )
    })
}

fn resolve_package_script_path(package_root: &Path, script_path: &str) -> Result<PathBuf, String> {
    let relative_script_path = Path::new(script_path);

    if relative_script_path.is_absolute() {
        return Err("Use a script path relative to the selected Move package.".to_string());
    }

    let script_path = package_root.join(relative_script_path);
    let script_path = script_path.canonicalize().map_err(|error| {
        format!(
            "Could not resolve bash script {}: {error}",
            script_path.display()
        )
    })?;

    if !script_path.starts_with(package_root) {
        return Err("Bash script must be inside the selected Move package.".to_string());
    }

    if !script_path.is_file() {
        return Err("Bash script path does not point to a file.".to_string());
    }

    Ok(script_path)
}

#[derive(Clone, Copy)]
enum PackageTreeMode {
    Detailed,
    Shallow,
}

fn build_package_tree(root_path: String, mode: PackageTreeMode) -> Result<PackageTree, String> {
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
    let close_project = MenuItemBuilder::with_id(CLOSE_PROJECT_MENU_ID, "Close Project")
        .accelerator("Cmd+Shift+W")
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
                &[
                    &close_project,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::close_window(app, None)?,
                ],
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
            } else if event.id().as_ref() == CLOSE_PROJECT_MENU_ID {
                let _ = app.emit(CLOSE_PROJECT_EVENT, ());
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            load_package_tree,
            load_package_tree_details,
            load_move_graphs,
            load_move_state_access_graph,
            load_file_preview,
            save_text_file,
            save_graph_png,
            build_move_package,
            run_security_command,
            run_movy_fuzz,
            run_security_script,
            analyze_move_package,
            load_move_bytecode_view,
            check_sui_adapter,
            get_sui_adapter_settings,
            save_sui_adapter_settings,
            load_project_metadata,
            save_project_metadata,
            list_ollama_models,
            chat_with_ollama,
            preload_ollama_model,
            stream_chat_with_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
