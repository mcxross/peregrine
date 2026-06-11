use crate::agent::legacy_core::config::Config;
use peregrine_helper_protocol::{HELPER_ENV_VAR, resolve_helper_executable};
use peregrine_mcp_protocol::{SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, resolve_server_executable};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, JsonObject},
    service::{RunningService, RunningServiceCancellationToken},
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

pub(crate) struct McpToolClient;

const MCP_STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

struct ToolCall {
    tool_name: &'static str,
    arguments: JsonObject,
    response_tx: mpsc::SyncSender<Result<Value, String>>,
}

enum WorkerMessage {
    Call(ToolCall),
    Shutdown(mpsc::SyncSender<()>),
}

struct McpClientWorker {
    message_tx: mpsc::Sender<WorkerMessage>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
    cancellation_token: Arc<Mutex<Option<RunningServiceCancellationToken>>>,
}

type RunningClient = RunningService<RoleClient, ()>;

impl McpToolClient {
    pub(crate) fn call_blocking<Args, Response>(
        workspace_root: &Path,
        tool_name: &'static str,
        arguments: &Args,
    ) -> Result<Response, String>
    where
        Args: Serialize,
        Response: DeserializeOwned,
    {
        let arguments = serialize_arguments(arguments)?;
        let worker = mcp_worker(workspace_root)?;
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        worker
            .message_tx
            .send(WorkerMessage::Call(ToolCall {
                tool_name,
                arguments,
                response_tx,
            }))
            .map_err(|_| "peregrine MCP client worker stopped".to_string())?;
        let value = response_rx
            .recv()
            .map_err(|_| "peregrine MCP client worker stopped without a response".to_string())??;
        decode_response(tool_name, value)
    }

    pub(crate) fn shutdown_all() {
        let workers = {
            let mut workers = workers()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            workers
                .drain()
                .map(|(_, worker)| worker)
                .collect::<Vec<_>>()
        };
        for worker in workers {
            worker.shutdown();
        }
    }
}

fn mcp_worker(workspace_root: &Path) -> Result<Arc<McpClientWorker>, String> {
    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let mut workers = workers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(worker) = workers.get(&workspace_root) {
        return Ok(worker.clone());
    }

    let application = crate::app::ApplicationRuntime::load(workspace_root.clone())
        .map_err(|error| format!("failed to load application config: {error}"))?;
    let worker = McpClientWorker::start(application.config(), workspace_root.clone())?;
    workers.insert(workspace_root, worker.clone());
    Ok(worker)
}

fn workers() -> &'static Mutex<HashMap<PathBuf, Arc<McpClientWorker>>> {
    static WORKERS: OnceLock<Mutex<HashMap<PathBuf, Arc<McpClientWorker>>>> = OnceLock::new();
    WORKERS.get_or_init(|| Mutex::new(HashMap::new()))
}

impl McpClientWorker {
    fn start(config: Arc<Config>, workspace_root: PathBuf) -> Result<Arc<Self>, String> {
        let (message_tx, message_rx) = mpsc::channel();
        let cancellation_token = Arc::new(Mutex::new(None));
        let worker_cancellation_token = cancellation_token.clone();
        let join_handle = thread::Builder::new()
            .name("peregrine-mcp-client".to_string())
            .spawn(move || {
                run_worker(
                    config,
                    workspace_root,
                    message_rx,
                    worker_cancellation_token,
                )
            })
            .map_err(|error| format!("failed to start MCP client worker: {error}"))?;
        Ok(Arc::new(Self {
            message_tx,
            join_handle: Mutex::new(Some(join_handle)),
            cancellation_token,
        }))
    }

    fn shutdown(&self) {
        if let Some(cancellation_token) = self
            .cancellation_token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            cancellation_token.cancel();
        }
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        let acknowledged = self
            .message_tx
            .send(WorkerMessage::Shutdown(response_tx))
            .is_ok()
            && response_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        if let Some(join_handle) = self
            .join_handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            && acknowledged
        {
            let _ = join_handle.join();
        }
    }
}

fn run_worker(
    config: Arc<Config>,
    workspace_root: PathBuf,
    message_rx: mpsc::Receiver<WorkerMessage>,
    cancellation_token: Arc<Mutex<Option<RunningServiceCancellationToken>>>,
) {
    let runtime = match crate::build_agent_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            fail_pending_calls(
                message_rx,
                format!("failed to start MCP client runtime: {error}"),
            );
            return;
        }
    };
    let mut client: Option<RunningClient> = None;
    let mut shutdown_tx = None;

    while let Ok(message) = message_rx.recv() {
        let call = match message {
            WorkerMessage::Call(call) => call,
            WorkerMessage::Shutdown(response_tx) => {
                shutdown_tx = Some(response_tx);
                break;
            }
        };
        if client.as_ref().is_none_or(RunningService::is_closed) {
            client = match runtime.block_on(connect(&config, &workspace_root)) {
                Ok(new_client) => {
                    *cancellation_token
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some(new_client.cancellation_token());
                    Some(new_client)
                }
                Err(error) => {
                    let _ = call.response_tx.send(Err(error));
                    continue;
                }
            };
        }

        let Some(running_client) = client.as_ref() else {
            let _ = call.response_tx.send(Err(
                "MCP client was unavailable after initialization".to_string()
            ));
            continue;
        };
        let result = runtime.block_on(call_tool(running_client, call.tool_name, call.arguments));
        if client
            .as_ref()
            .is_some_and(|client| client.is_closed() || client.is_transport_closed())
        {
            client = None;
            cancellation_token
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }
        let _ = call.response_tx.send(result);
    }

    if let Some(client) = client {
        let _ = runtime.block_on(client.cancel());
    }
    cancellation_token
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(response_tx) = shutdown_tx {
        let _ = response_tx.send(());
    }
}

fn fail_pending_calls(message_rx: mpsc::Receiver<WorkerMessage>, error: String) {
    while let Ok(message) = message_rx.recv() {
        match message {
            WorkerMessage::Call(call) => {
                let _ = call.response_tx.send(Err(error.clone()));
            }
            WorkerMessage::Shutdown(response_tx) => {
                let _ = response_tx.send(());
                return;
            }
        }
    }
}

async fn connect(config: &Config, workspace_root: &Path) -> Result<RunningClient, String> {
    let executable = resolve_server_executable();
    let helper = resolve_helper_executable()
        .map_err(|error| format!("failed to resolve Peregrine helper for MCP server: {error}"))?;
    let transport = TokioChildProcess::new(tokio::process::Command::new(&executable).configure(
        |command| {
            command
                .current_dir(workspace_root)
                .env(HELPER_ENV_VAR, helper)
                .env(
                    SUI_ADAPTER_SOURCE_ENV,
                    config.sui_security_tools.adapter.source.as_str(),
                )
                .env("NO_COLOR", "1")
                .env("CLICOLOR", "0")
                .env("TERM", "dumb");
            if let Some(cli_path) = config.sui_security_tools.adapter.cli_path.as_deref() {
                command.env(SUI_CLI_PATH_ENV, cli_path);
            }
        },
    ))
    .map_err(|error| {
        format!(
            "failed to start {} at {}: {error}",
            peregrine_mcp_protocol::SERVER_NAME,
            executable.display()
        )
    })?;
    tokio::time::timeout(MCP_STARTUP_TIMEOUT, ().serve(transport))
        .await
        .map_err(|_| {
            format!(
                "{} MCP server did not initialize within {}s",
                peregrine_mcp_protocol::SERVER_NAME,
                MCP_STARTUP_TIMEOUT.as_secs()
            )
        })?
        .map_err(|error| {
            format!(
                "failed to initialize {} MCP server: {error}",
                peregrine_mcp_protocol::SERVER_NAME
            )
        })
}

async fn call_tool(
    client: &RunningClient,
    tool_name: &'static str,
    arguments: JsonObject,
) -> Result<Value, String> {
    let result = client
        .call_tool(CallToolRequestParams::new(tool_name).with_arguments(arguments))
        .await
        .map_err(|error| format!("MCP tool `{tool_name}` failed: {error}"))?;
    if result.is_error == Some(true) {
        return Err(result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("MCP tool returned an error")
            .to_string());
    }
    result
        .structured_content
        .ok_or_else(|| format!("MCP tool `{tool_name}` returned no structured result"))
}

fn serialize_arguments(arguments: &impl Serialize) -> Result<JsonObject, String> {
    let arguments = serde_json::to_value(arguments)
        .map_err(|error| format!("failed to serialize MCP tool arguments: {error}"))?;
    serde_json::from_value(arguments)
        .map_err(|error| format!("MCP tool arguments must be an object: {error}"))
}

fn decode_response<Response>(tool_name: &str, value: Value) -> Result<Response, String>
where
    Response: DeserializeOwned,
{
    serde_json::from_value(value)
        .map_err(|error| format!("failed to decode MCP tool `{tool_name}` response: {error}"))
}
