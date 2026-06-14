use peregrine_mcp_client::McpClientRuntime;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::commands::agent_server::session::AgentServerSessions;

#[derive(Default)]
pub(crate) struct IndexerCommandState {
    pub(crate) active_db_path: Mutex<Option<PathBuf>>,
    pub(crate) canceled_runs: Mutex<HashSet<String>>,
}

#[derive(Default)]
pub(crate) struct MoveAnalyzerMcpState {
    pub(crate) runtimes: Mutex<HashMap<PathBuf, Arc<McpClientRuntime>>>,
}

#[derive(Clone, Default)]
pub(crate) struct AgentServerCommandState {
    pub(crate) sessions: Arc<AgentServerSessions>,
}
