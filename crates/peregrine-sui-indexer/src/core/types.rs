use serde::{Deserialize, Serialize};

use super::{FieldId, ModuleId, PackageId, SourceSpan, TypeId};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TypeKind {
    Struct,
    Enum,
    Native,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub id: TypeId,
    pub package_id: PackageId,
    pub module_id: ModuleId,
    pub name: String,
    pub full_name: String,
    pub kind: TypeKind,
    pub abilities: Vec<String>,
    pub type_parameters: Vec<String>,
    pub fields: Vec<FieldInfo>,
    pub docs: Option<String>,
    pub attributes: Vec<String>,
    pub source_span: SourceSpan,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    pub id: FieldId,
    pub package_id: PackageId,
    pub module_id: ModuleId,
    pub type_id: TypeId,
    pub name: String,
    pub type_name: String,
    pub source_span: SourceSpan,
}
