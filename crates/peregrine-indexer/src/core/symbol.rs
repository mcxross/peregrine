use serde::{Deserialize, Serialize};

use super::{FunctionId, ModuleId, PackageId, SourceSpan};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PackageRole {
    Root,
    Framework,
    StandardLibrary,
    OracleDependency,
    ProtocolDependency,
    TokenDependency,
    TransitiveDependency,
    UnknownDependency,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PackageStatus {
    Indexed,
    PartialWithDiagnostics,
    FailedToCompile,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum MaterializedStatus {
    PointerOnly,
    RootCard,
    DirectDependencyCard,
    ExpandedModule,
    ExpandedSymbol,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    pub id: PackageId,
    pub name: String,
    pub root_path: String,
    pub manifest_path: String,
    pub role: PackageRole,
    pub compiler_version: Option<String>,
    pub package_hash: String,
    pub status: PackageStatus,
    pub indexed_at: i64,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryArtifact {
    pub id: String,
    pub package_id: PackageId,
    pub package_alias: String,
    pub module_name: String,
    pub summary_path: String,
    pub content_hash: String,
    pub schema_version: Option<String>,
    pub role: PackageRole,
    pub materialized_status: MaterializedStatus,
    pub last_seen_at: i64,
    pub card_json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressMapping {
    pub id: String,
    pub package_id: PackageId,
    pub alias: String,
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfo {
    pub id: ModuleId,
    pub package_id: PackageId,
    pub summary_artifact_id: Option<String>,
    pub file_id: Option<String>,
    pub address: String,
    pub name: String,
    pub full_name: String,
    pub immediate_dependencies: Vec<String>,
    pub docs: Option<String>,
    pub attributes: Vec<String>,
    pub source_span: SourceSpan,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum FunctionVisibility {
    Public,
    PublicFriend,
    PublicPackage,
    Private,
    Native,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionParameter {
    pub id: String,
    pub name: Option<String>,
    pub type_name: String,
    pub index: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionInfo {
    pub id: FunctionId,
    pub package_id: PackageId,
    pub module_id: ModuleId,
    pub name: String,
    pub full_name: String,
    pub visibility: FunctionVisibility,
    pub is_entry: bool,
    pub is_native: bool,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<FunctionParameter>,
    pub returns: Vec<String>,
    pub acquires: Vec<String>,
    pub docs: Option<String>,
    pub attributes: Vec<String>,
    pub source_span: SourceSpan,
}
