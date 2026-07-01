mod adapter;
mod analysis;
mod artifacts;
mod cache;
mod command;
mod dynamic;
mod error;
mod graphs;
mod server;
mod transport;

use anyhow::Context;
use peregrine_sui_mcp_protocol::{
    SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings, SuiAdapterSource,
};
use rmcp::{ServiceExt, transport::stdio};
pub use server::PeregrineMcpServer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    Stdio,
    Sse { port: u16 },
}

pub fn run_server(transport: TransportKind) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .context("create Peregrine MCP runtime")?
        .block_on(run_server_async(transport))
}

async fn run_server_async(transport: TransportKind) -> anyhow::Result<()> {
    let workspace_root = std::env::current_dir().context("resolve MCP server workspace")?;
    let adapter_settings = SuiAdapterSettings {
        source: match std::env::var(SUI_ADAPTER_SOURCE_ENV).as_deref() {
            Ok("system") => SuiAdapterSource::System,
            _ => SuiAdapterSource::Bundled,
        },
        cli_path: std::env::var(SUI_CLI_PATH_ENV).ok(),
    };
    let service =
        PeregrineMcpServer::new(workspace_root)?.with_adapter_settings(adapter_settings)?;

    match transport {
        TransportKind::Stdio => {
            let service = service
                .serve(stdio())
                .await
                .context("start Peregrine MCP server over stdio")?;
            service
                .waiting()
                .await
                .context("run Peregrine MCP server over stdio")?;
        }
        TransportKind::Sse { port } => {
            let server = service;

            let srv = server.clone();
            tokio::spawn(async move {
                let timeout_duration = std::time::Duration::from_secs(10 * 60);
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    let last_activity =
                        srv.last_activity.load(std::sync::atomic::Ordering::Relaxed);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if now.saturating_sub(last_activity) >= timeout_duration.as_secs() {
                        tracing::info!("Server has been idle for 10 minutes. Shutting down.");
                        std::process::exit(0);
                    }
                }
            });

            transport::run_sse_server(port, move || Ok(server.clone())).await?;
        }
    }

    Ok(())
}
