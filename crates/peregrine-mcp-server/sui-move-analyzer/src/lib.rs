mod lsp;
mod server;

use anyhow::Context;
use peregrine_sui_move_analyzer_mcp_protocol::{
    ADAPTER_SOURCE_ENV, BINARY_PATH_ENV, MoveAnalyzerAdapterSettings, MoveAnalyzerAdapterSource,
};
use rmcp::{ServiceExt, transport::stdio};
pub use server::SuiMoveAnalyzerMcpServer;

pub fn run_stdio() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .context("create Sui Move Analyzer MCP runtime")?
        .block_on(run_stdio_async())
}

async fn run_stdio_async() -> anyhow::Result<()> {
    let workspace_root = std::env::current_dir().context("resolve MCP server workspace")?;
    let settings = MoveAnalyzerAdapterSettings {
        source: match std::env::var(ADAPTER_SOURCE_ENV).as_deref() {
            Ok("system") => MoveAnalyzerAdapterSource::System,
            _ => MoveAnalyzerAdapterSource::Bundled,
        },
        binary_path: std::env::var(BINARY_PATH_ENV).ok(),
    };
    let service = SuiMoveAnalyzerMcpServer::new(workspace_root, settings)?
        .serve(stdio())
        .await
        .context("start Sui Move Analyzer MCP server")?;
    service
        .waiting()
        .await
        .context("run Sui Move Analyzer MCP server")?;
    Ok(())
}
