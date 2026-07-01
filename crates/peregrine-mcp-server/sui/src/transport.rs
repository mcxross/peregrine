use anyhow::Context;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::{StreamableHttpServerConfig, StreamableHttpService},
};
use std::sync::Arc;
use tokio::net::TcpListener;

pub async fn run_sse_server(
    port: u16,
    service_factory: impl Fn() -> anyhow::Result<crate::PeregrineMcpServer> + Send + Sync + 'static,
) -> anyhow::Result<()> {
    let session_manager = Arc::new(LocalSessionManager::default());
    let config = StreamableHttpServerConfig::default();

    let service = StreamableHttpService::new(
        move || {
            service_factory()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        },
        session_manager,
        config,
    );

    let app = axum::Router::new().fallback_service(service);

    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("failed to bind TCP listener on port {}", port))?;

    tracing::info!(
        "Starting Peregrine MCP Server over SSE on 127.0.0.1:{}",
        port
    );

    axum::serve(listener, app.into_make_service())
        .await
        .context("axum server error")?;

    Ok(())
}
