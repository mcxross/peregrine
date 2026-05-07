use crate::SuiAdapterSource;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiAdapterStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub install_hint: Option<String>,
    pub active_source: Option<SuiAdapterSource>,
    pub preferred_source: SuiAdapterSource,
    pub resolved_path: Option<String>,
    pub bundled: SuiAdapterSourceStatus,
    pub system: SuiAdapterSourceStatus,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiAdapterSourceStatus {
    pub source: SuiAdapterSource,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub error: Option<String>,
}
