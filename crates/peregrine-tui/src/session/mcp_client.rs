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
