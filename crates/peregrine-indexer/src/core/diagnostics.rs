use serde::{Deserialize, Serialize};

use super::{DiagnosticId, PackageId, SourceSpan};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub id: DiagnosticId,
    pub package_id: PackageId,
    pub severity: DiagnosticSeverity,
    pub source: String,
    pub message: String,
    pub source_span: SourceSpan,
    pub metadata_json: Option<serde_json::Value>,
}
