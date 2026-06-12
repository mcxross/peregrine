//! Shared MCP server resolution and direct client runtime.

mod resolver;
mod runtime;

pub use resolver::McpClientOptions;
pub use resolver::McpExecutionOrigin;
pub use resolver::ResolvedMcpConfig;
pub use resolver::default_peregrine_server;
pub use resolver::resolve_mcp_config;
pub use runtime::McpClientHandle;
pub use runtime::McpClientRuntime;
