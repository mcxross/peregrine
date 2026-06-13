use serde::{Deserialize, Serialize};

use super::{FileId, SourcePrecision, SummaryArtifactId};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    pub file_id: Option<FileId>,
    pub summary_artifact_id: Option<SummaryArtifactId>,
    pub start_line: Option<u32>,
    pub start_col: Option<u32>,
    pub end_line: Option<u32>,
    pub end_col: Option<u32>,
    pub precision: SourcePrecision,
}

impl SourceSpan {
    pub fn unknown() -> Self {
        Self::default()
    }

    pub fn summary_artifact(summary_artifact_id: SummaryArtifactId) -> Self {
        Self {
            summary_artifact_id: Some(summary_artifact_id),
            precision: SourcePrecision::SummaryArtifact,
            ..Self::default()
        }
    }
}
