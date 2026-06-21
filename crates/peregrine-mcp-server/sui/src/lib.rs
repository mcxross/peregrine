mod adapter;
mod analysis;
mod artifacts;
mod cache;
mod command;
mod dynamic;
mod error;
mod graphs;
mod server;

use anyhow::Context;
use peregrine_sui_mcp_protocol::{
    SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings, SuiAdapterSource,
};
use rmcp::{ServiceExt, transport::stdio};
pub use server::PeregrineMcpServer;

pub fn run_stdio() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create Peregrine MCP runtime")?
        .block_on(run_stdio_async())
}

async fn run_stdio_async() -> anyhow::Result<()> {
    let workspace_root = std::env::current_dir().context("resolve MCP server workspace")?;
    let adapter_settings = SuiAdapterSettings {
        source: match std::env::var(SUI_ADAPTER_SOURCE_ENV).as_deref() {
            Ok("system") => SuiAdapterSource::System,
            _ => SuiAdapterSource::Bundled,
        },
        cli_path: std::env::var(SUI_CLI_PATH_ENV).ok(),
    };
    let service = PeregrineMcpServer::new(workspace_root)?
        .with_adapter_settings(adapter_settings)?
        .serve(stdio())
        .await
        .context("start Peregrine MCP server")?;
    service
        .waiting()
        .await
        .context("run Peregrine MCP server")?;
    Ok(())
}
