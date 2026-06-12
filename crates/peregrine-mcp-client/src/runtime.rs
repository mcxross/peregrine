use crate::{McpClientOptions, ResolvedMcpConfig, resolve_mcp_config};
use http::{HeaderName, HeaderValue};
use peregrine_config::{McpServerConfig, McpServerTransportConfig};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, JsonObject},
    service::{RunningService, RunningServiceCancellationToken},
    transport::{
        ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::sync::oneshot;

type RunningClient = RunningService<RoleClient, ()>;

pub struct McpClientRuntime {
    worker: Arc<ClientWorker>,
}

#[derive(Clone)]
pub struct McpClientHandle {
    message_tx: mpsc::Sender<WorkerMessage>,
}

struct ClientWorker {
    message_tx: mpsc::Sender<WorkerMessage>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
    cancellation_tokens: Arc<Mutex<HashMap<String, RunningServiceCancellationToken>>>,
}

struct ToolCall {
    server: String,
    tool: String,
    arguments: Value,
    response_tx: oneshot::Sender<Result<Value, String>>,
}

enum WorkerMessage {
    Call(ToolCall),
    Shutdown(mpsc::SyncSender<()>),
}

impl McpClientRuntime {
    pub fn load(options: McpClientOptions) -> Result<Self, String> {
        Self::from_resolved(
            resolve_mcp_config(options)
                .map_err(|error| format!("failed to resolve MCP configuration: {error}"))?,
        )
    }

    pub fn from_resolved(config: ResolvedMcpConfig) -> Result<Self, String> {
        Ok(Self {
            worker: ClientWorker::start(config)?,
        })
    }

    pub fn handle(&self) -> McpClientHandle {
        McpClientHandle {
            message_tx: self.worker.message_tx.clone(),
        }
    }

    pub fn shutdown(&self) {
        self.worker.shutdown();
    }
}

impl Drop for McpClientRuntime {
    fn drop(&mut self) {
        self.worker.shutdown();
    }
}

impl McpClientHandle {
    pub async fn call(
        &self,
        server: impl Into<String>,
        tool: impl Into<String>,
        arguments: Value,
    ) -> Result<Value, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.message_tx
            .send(WorkerMessage::Call(ToolCall {
                server: server.into(),
                tool: tool.into(),
                arguments,
                response_tx,
            }))
            .map_err(|_| "MCP client runtime has stopped".to_string())?;
        response_rx
            .await
            .map_err(|_| "MCP client runtime stopped without a response".to_string())?
    }

    pub fn call_blocking<Args, Response>(
        &self,
        server: &str,
        tool: &str,
        arguments: &Args,
    ) -> Result<Response, String>
    where
        Args: Serialize,
        Response: DeserializeOwned,
    {
        let arguments = serde_json::to_value(arguments)
            .map_err(|error| format!("failed to serialize MCP tool arguments: {error}"))?;
        let (response_tx, response_rx) = oneshot::channel();
        self.message_tx
            .send(WorkerMessage::Call(ToolCall {
                server: server.to_string(),
                tool: tool.to_string(),
                arguments,
                response_tx,
            }))
            .map_err(|_| "MCP client runtime has stopped".to_string())?;
        let value = response_rx
            .blocking_recv()
            .map_err(|_| "MCP client runtime stopped without a response".to_string())??;
        serde_json::from_value(value)
            .map_err(|error| format!("failed to decode MCP tool `{tool}` response: {error}"))
    }
}

impl ClientWorker {
    fn start(config: ResolvedMcpConfig) -> Result<Arc<Self>, String> {
        let (message_tx, message_rx) = mpsc::channel();
        let cancellation_tokens = Arc::new(Mutex::new(HashMap::new()));
        let worker_cancellation_tokens = cancellation_tokens.clone();
        let join_handle = thread::Builder::new()
            .name(format!("peregrine-mcp-{}", config.origin.as_str()))
            .spawn(move || run_worker(config, message_rx, worker_cancellation_tokens))
            .map_err(|error| format!("failed to start MCP client runtime: {error}"))?;
        Ok(Arc::new(Self {
            message_tx,
            join_handle: Mutex::new(Some(join_handle)),
            cancellation_tokens,
        }))
    }

    fn shutdown(&self) {
        for cancellation_token in self
            .cancellation_tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain()
            .map(|(_, token)| token)
        {
            cancellation_token.cancel();
        }
        let mut join_handle = self
            .join_handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(join_handle) = join_handle.take() else {
            return;
        };
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        let _ = self.message_tx.send(WorkerMessage::Shutdown(response_tx));
        if response_rx.recv_timeout(Duration::from_secs(5)).is_ok() {
            let _ = join_handle.join();
        }
    }
}

fn run_worker(
    config: ResolvedMcpConfig,
    message_rx: mpsc::Receiver<WorkerMessage>,
    cancellation_tokens: Arc<Mutex<HashMap<String, RunningServiceCancellationToken>>>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            reject_calls(message_rx, format!("failed to create MCP runtime: {error}"));
            return;
        }
    };
    let mut clients = HashMap::<String, RunningClient>::new();
    while let Ok(message) = message_rx.recv() {
        match message {
            WorkerMessage::Call(call) => {
                let result = runtime.block_on(execute_call(
                    &config,
                    &mut clients,
                    &cancellation_tokens,
                    &call,
                ));
                let _ = call.response_tx.send(result);
            }
            WorkerMessage::Shutdown(response_tx) => {
                for (_, client) in clients.drain() {
                    let _ = runtime.block_on(client.cancel());
                }
                let _ = response_tx.send(());
                return;
            }
        }
    }
}

async fn execute_call(
    resolved: &ResolvedMcpConfig,
    clients: &mut HashMap<String, RunningClient>,
    cancellation_tokens: &Mutex<HashMap<String, RunningServiceCancellationToken>>,
    call: &ToolCall,
) -> Result<Value, String> {
    let server = resolved
        .servers
        .get(&call.server)
        .ok_or_else(|| format!("MCP server `{}` is not configured", call.server))?;
    validate_call(server, &call.server, &call.tool)?;
    if clients
        .get(&call.server)
        .is_none_or(|client| client.is_closed() || client.is_transport_closed())
    {
        let client = connect(server, &resolved.workspace_root, &call.server).await?;
        cancellation_tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(call.server.clone(), client.cancellation_token());
        clients.insert(call.server.clone(), client);
    }
    let client = clients
        .get(&call.server)
        .ok_or_else(|| format!("MCP server `{}` was unavailable after startup", call.server))?;
    let arguments: JsonObject = serde_json::from_value(call.arguments.clone())
        .map_err(|error| format!("MCP tool arguments must be an object: {error}"))?;
    let request =
        client.call_tool(CallToolRequestParams::new(call.tool.clone()).with_arguments(arguments));
    let result = match server.tool_timeout_sec {
        Some(timeout) => tokio::time::timeout(timeout, request)
            .await
            .map_err(|_| format!("MCP tool `{}/{}` timed out", call.server, call.tool))?,
        None => request.await,
    }
    .map_err(|error| format!("MCP tool `{}/{}` failed: {error}", call.server, call.tool))?;
    if result.is_error == Some(true) {
        return Err(result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("MCP tool returned an error")
            .to_string());
    }
    result.structured_content.ok_or_else(|| {
        format!(
            "MCP tool `{}/{}` returned no structured result",
            call.server, call.tool
        )
    })
}

fn validate_call(server: &McpServerConfig, server_name: &str, tool: &str) -> Result<(), String> {
    if !server.enabled {
        return Err(format!("MCP server `{server_name}` is disabled"));
    }
    if server
        .enabled_tools
        .as_ref()
        .is_some_and(|tools| !tools.iter().any(|enabled| enabled == tool))
        || server
            .disabled_tools
            .as_ref()
            .is_some_and(|tools| tools.iter().any(|disabled| disabled == tool))
    {
        return Err(format!(
            "MCP tool `{server_name}/{tool}` is disabled by configuration"
        ));
    }
    Ok(())
}

async fn connect(
    server: &McpServerConfig,
    workspace_root: &std::path::Path,
    server_name: &str,
) -> Result<RunningClient, String> {
    if !server.is_local_environment() {
        return Err(format!(
            "direct MCP runtime cannot launch environment `{}` for server `{server_name}`",
            server.environment_id
        ));
    }
    let startup_timeout = server
        .startup_timeout_sec
        .unwrap_or(Duration::from_secs(10));
    match &server.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            let mut inherited = env.clone().unwrap_or_default();
            for variable in env_vars {
                if let Some(value) = std::env::var_os(variable.name()) {
                    inherited.insert(variable.name().to_string(), value.to_string_lossy().into());
                }
            }
            let transport = TokioChildProcess::new(
                tokio::process::Command::new(command).configure(|process| {
                    process
                        .args(args)
                        .current_dir(cwd.as_deref().unwrap_or(workspace_root))
                        .envs(inherited);
                }),
            )
            .map_err(|error| format!("failed to start MCP server `{server_name}`: {error}"))?;
            initialize(server_name, startup_timeout, transport).await
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => {
            let mut headers = http_headers.clone().unwrap_or_default();
            for (name, variable) in env_http_headers.as_ref().into_iter().flatten() {
                let value = std::env::var(variable).map_err(|_| {
                    format!("environment variable `{variable}` for HTTP header `{name}` is unset")
                })?;
                headers.insert(name.clone(), value);
            }
            let headers = headers
                .into_iter()
                .map(|(name, value)| {
                    Ok((
                        name.parse::<HeaderName>()
                            .map_err(|error| format!("invalid HTTP header `{name}`: {error}"))?,
                        value.parse::<HeaderValue>().map_err(|error| {
                            format!("invalid value for HTTP header `{name}`: {error}")
                        })?,
                    ))
                })
                .collect::<Result<HashMap<_, _>, String>>()?;
            let mut transport_config =
                StreamableHttpClientTransportConfig::with_uri(url.clone()).custom_headers(headers);
            if let Some(variable) = bearer_token_env_var {
                let token = std::env::var(variable).map_err(|_| {
                    format!("bearer token environment variable `{variable}` is unset")
                })?;
                transport_config = transport_config.auth_header(token);
            }
            let transport = StreamableHttpClientTransport::from_config(transport_config);
            initialize(server_name, startup_timeout, transport).await
        }
    }
}

async fn initialize<T, E, A>(
    server_name: &str,
    startup_timeout: Duration,
    transport: T,
) -> Result<RunningClient, String>
where
    T: rmcp::transport::IntoTransport<RoleClient, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    tokio::time::timeout(startup_timeout, ().serve(transport))
        .await
        .map_err(|_| {
            format!(
                "MCP server `{server_name}` did not initialize within {}s",
                startup_timeout.as_secs()
            )
        })?
        .map_err(|error| format!("failed to initialize MCP server `{server_name}`: {error}"))
}

fn reject_calls(message_rx: mpsc::Receiver<WorkerMessage>, error: String) {
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
