use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum AuditTargetParams {
    LocalPackage {
        #[serde(rename = "chainId")]
        #[ts(rename = "chainId")]
        chain_id: String,
        path: String,
        #[ts(optional = nullable, type = "Record<string, unknown>")]
        metadata: Option<HashMap<String, JsonValue>>,
    },
    RemotePackage {
        #[serde(rename = "chainId")]
        #[ts(rename = "chainId")]
        chain_id: String,
        #[serde(rename = "networkId")]
        #[ts(rename = "networkId")]
        network_id: String,
        #[serde(rename = "packageRef")]
        #[ts(rename = "packageRef")]
        package_ref: String,
        #[serde(rename = "sourceUri")]
        #[ts(rename = "sourceUri")]
        #[ts(optional = nullable)]
        source_uri: Option<String>,
        #[serde(rename = "stateRef")]
        #[ts(rename = "stateRef")]
        #[ts(optional = nullable)]
        state_ref: Option<String>,
        #[ts(optional = nullable, type = "Record<string, unknown>")]
        metadata: Option<HashMap<String, JsonValue>>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditProfileParams {
    #[ts(type = "number")]
    pub model_token_budget: i64,
    #[ts(type = "number")]
    pub wall_time_seconds: i64,
    pub max_hypotheses: u32,
    pub max_dependency_depth: u32,
    pub max_dependency_packages: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditPreflightParams {
    pub target: AuditTargetParams,
    #[ts(optional = nullable)]
    pub profile: Option<AuditProfileParams>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditPreflightResponse {
    #[ts(type = "unknown")]
    pub plan: JsonValue,
    pub diagnostics: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditPlanStoreParams {
    #[ts(type = "unknown")]
    pub plan: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditPlanStoreResponse {
    pub fingerprint: String,
    #[ts(type = "unknown")]
    pub plan: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditStartParams {
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditStartResponse {
    #[ts(type = "unknown")]
    pub run: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditReadParams {
    pub audit_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditReadResponse {
    #[ts(type = "unknown")]
    pub run: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditListParams {
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditListResponse {
    #[ts(type = "unknown[]")]
    pub data: Vec<JsonValue>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditLifecycleParams {
    pub audit_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditPauseResponse {
    #[ts(type = "unknown")]
    pub run: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditResumeResponse {
    #[ts(type = "unknown")]
    pub run: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditCancelResponse {
    #[ts(type = "unknown")]
    pub run: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditDeleteResponse {
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditUpdatedNotification {
    pub audit_id: String,
    #[ts(type = "unknown")]
    pub run: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AuditDiagnosticNotification {
    pub audit_id: Option<String>,
    pub message: String,
}
