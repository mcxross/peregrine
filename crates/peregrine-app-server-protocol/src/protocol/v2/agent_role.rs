use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::config::ConfigWriteResponse;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum AgentRoleSource {
    BuiltIn,
    Configured,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum AgentRoleSaveScope {
    Global,
    Local,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleSummary {
    pub name: String,
    pub description: Option<String>,
    pub source: AgentRoleSource,
    pub config_file: Option<String>,
    pub nickname_candidates: Option<Vec<String>>,
    pub overrides_built_in: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleListParams {
    /// Optional working directory used to resolve project config layers.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleListResponse {
    pub roles: Vec<AgentRoleSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleReadParams {
    pub name: String,
    /// Optional working directory used to resolve project config layers.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    /// When true, prepare a new role template instead of requiring an existing role.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub create: bool,
    #[ts(optional = nullable)]
    pub scope: Option<AgentRoleSaveScope>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleReadResponse {
    pub name: String,
    pub source: Option<AgentRoleSource>,
    pub scope: AgentRoleSaveScope,
    pub config_file: String,
    pub global_config_file: String,
    pub directory_config_file: Option<String>,
    pub editable_content: String,
    pub save_config_file: String,
    pub save_config_version: Option<String>,
    pub create: bool,
    pub overrides_built_in: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleWriteParams {
    pub name: String,
    pub editable_content: String,
    pub scope: AgentRoleSaveScope,
    /// Optional working directory used to resolve project config layers.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    #[ts(optional = nullable)]
    pub expected_version: Option<String>,
    /// When true, reject writes if the effective role already exists.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub create: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct AgentRoleWriteResponse {
    pub role: AgentRoleSummary,
    pub config_file: String,
    pub config_write: ConfigWriteResponse,
}
