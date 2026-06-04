//! Native Anthropic provider runtime.
//!
//! This crate owns Anthropic-specific request conversion, HTTP/SSE streaming,
//! and the runtime `ModelProvider` implementation. The TUI still talks only to
//! the app-server; core selects this crate through the provider registry.

mod client;
mod model_provider;
mod request;
mod sse;

pub use client::AnthropicMessagesClient;
pub use client::AnthropicMessagesOptions;
pub use model_provider::AnthropicModelProvider;
pub use request::AnthropicContentBlock;
pub use request::AnthropicMessage;
pub use request::AnthropicMessagesApiRequest;
pub use request::AnthropicTool;
pub use request::AnthropicToolChoice;
