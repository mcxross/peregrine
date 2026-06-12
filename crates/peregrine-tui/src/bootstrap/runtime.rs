use std::io;

pub const AGENT_TOKIO_WORKER_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

pub fn build_agent_runtime() -> io::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    // Match the upstream CLI runtime. The embedded app-server has large debug async
    // state machines, and Tokio's default worker stack can overflow on startup.
    builder.thread_stack_size(AGENT_TOKIO_WORKER_STACK_SIZE_BYTES);
    builder.build().map_err(io::Error::other)
}
