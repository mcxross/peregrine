use super::MoveAnalyzerAdapterSource;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MoveAnalyzerAdapterStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub install_hint: Option<String>,
    pub active_source: Option<MoveAnalyzerAdapterSource>,
    pub preferred_source: MoveAnalyzerAdapterSource,
    pub resolved_path: Option<String>,
    pub bundled: MoveAnalyzerAdapterSourceStatus,
    pub system: MoveAnalyzerAdapterSourceStatus,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MoveAnalyzerAdapterSourceStatus {
    pub source: MoveAnalyzerAdapterSource,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub error: Option<String>,
}
