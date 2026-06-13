use serde::{Deserialize, Serialize};

use super::{OperationId, PackageId, SourceSpan};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum EdgeType {
    Contains,
    Imports,
    Friends,
    DependsOnPackage,
    DependsOnModule,
    DefinesType,
    DefinesFunction,
    HasField,
    HasParameter,
    HasLocal,
    HasBlock,
    HasOperation,
    Calls,
    ReferencesType,
    ReadsField,
    WritesField,
    BorrowsField,
    BorrowsFieldMut,
    PacksType,
    UnpacksType,
    ReturnsType,
    AcceptsType,
    ControlFlow,
    DataFlow,
    AnalysisRelation,
    HasSummaryArtifact,
    MaterializedFromSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    pub package_id: PackageId,
    pub from_id: String,
    pub to_id: String,
    pub edge_type: EdgeType,
    pub operation_id: Option<OperationId>,
    pub source_span: SourceSpan,
    pub metadata_json: Option<serde_json::Value>,
}
