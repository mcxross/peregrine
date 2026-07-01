use peregrine_mcp_client::{
    McpClientHandle, McpClientOptions, McpClientRuntime, McpExecutionOrigin,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

pub(crate) struct McpToolClient;

struct WorkspaceClient {
    runtime: McpClientRuntime,
    handle: McpClientHandle,
}

impl McpToolClient {
    pub(crate) fn call_blocking<Args, Response>(
        workspace_root: &Path,
        tool_name: &str,
        arguments: &Args,
    ) -> Result<Response, String>
    where
        Args: Serialize,
        Response: DeserializeOwned,
    {
        workspace_client(workspace_root)?.handle.call_blocking(
            peregrine_sui_mcp_protocol::SERVER_NAME,
            tool_name,
            arguments,
        )
    }

    pub(crate) fn shutdown_all() {
        let clients = {
            let mut clients = clients()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            clients
                .drain()
                .map(|(_, client)| client)
                .collect::<Vec<_>>()
        };
        for client in clients {
            client.runtime.shutdown();
        }
    }
}

fn workspace_client(workspace_root: &Path) -> Result<Arc<WorkspaceClient>, String> {
    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let mut clients = clients()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(client) = clients.get(&workspace_root) {
        return Ok(client.clone());
    }

    let peregrine_home = peregrine_utils_home_dir::find_peregrine_home()
        .map_err(|error| format!("failed to resolve PEREGRINE_HOME: {error}"))?;

    let config_path = peregrine_home.join(peregrine_config::CONFIG_TOML_FILE);
    let config: peregrine_config::config_toml::ConfigToml = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|raw| toml::from_str(&raw).ok())
        .unwrap_or_default();

    if let Some(port) = config.sui_mcp_server_port {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            let self_exe = std::env::current_exe().ok();
            let server_executable = peregrine_sui_mcp_protocol::resolve_server_executable_from(
                self_exe.as_deref(),
                std::env::var_os(peregrine_sui_mcp_protocol::SERVER_PATH_ENV),
                std::env::var_os("PATH"),
            );

            let _ = std::process::Command::new(server_executable)
                .arg("--transport")
                .arg(format!("sse:{port}"))
                .spawn();

            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    let runtime = McpClientRuntime::load(McpClientOptions::new(
        workspace_root.clone(),
        peregrine_home.to_path_buf(),
        McpExecutionOrigin::Workbench,
    ))?;
    let client = Arc::new(WorkspaceClient {
        handle: runtime.handle(),
        runtime,
    });
    clients.insert(workspace_root, client.clone());
    Ok(client)
}

fn clients() -> &'static Mutex<HashMap<PathBuf, Arc<WorkspaceClient>>> {
    static CLIENTS: OnceLock<Mutex<HashMap<PathBuf, Arc<WorkspaceClient>>>> = OnceLock::new();
    CLIENTS.get_or_init(|| Mutex::new(HashMap::new()))
}
