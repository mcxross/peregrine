use anyhow::Context;
use peregrine_sui_mcp_protocol::{
    SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings, SuiAdapterSource,
};
use peregrine_sui_mcp_server::PeregrineMcpServer;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
