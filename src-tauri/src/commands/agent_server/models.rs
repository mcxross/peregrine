use super::target::AgentServerTargetConfig;
use peregrine_app_server_protocol::{
    ModelListResponse, ModelProviderListResponse, RequestId, Result as JsonRpcResult, Thread,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerStartRequest {
    pub(crate) session_id: String,
    pub(crate) agent_name: String,
    pub(crate) agent_role: Option<String>,
    pub(crate) agent_instructions: String,
    pub(crate) workflow_name: String,
    pub(crate) prompt: String,
    pub(crate) cwd: Option<PathBuf>,
    #[serde(default)]
    pub(crate) workspace_roots: Vec<PathBuf>,
    #[serde(default)]
    pub(crate) target: AgentServerTargetConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerStartResponse {
    pub(crate) session_id: String,
    pub(crate) thread_id: String,
    pub(crate) thread: Thread,
    pub(crate) turn_id: String,
    pub(crate) model: String,
    pub(crate) model_provider: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerTurnRequest {
    pub(crate) session_id: String,
    pub(crate) prompt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerTurnResponse {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerTurnInterruptRequest {
    pub(crate) session_id: String,
    pub(crate) turn_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerStopRequest {
    pub(crate) session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerResolveRequest {
    pub(crate) session_id: String,
    pub(crate) request_id: RequestId,
    pub(crate) result: JsonRpcResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerRejectRequest {
    pub(crate) session_id: String,
    pub(crate) request_id: RequestId,
    pub(crate) message: String,
    pub(crate) code: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerModelListRequest {
    pub(crate) cwd: Option<PathBuf>,
    #[serde(default)]
    pub(crate) target: AgentServerTargetConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerModelListResponse {
    pub(crate) models: ModelListResponse,
    pub(crate) providers: ModelProviderListResponse,
}
