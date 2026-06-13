use serde::{Deserialize, Serialize};

use super::{BasicBlockId, FunctionId, LocalId, OperationId, PackageId, SourceSpan};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OperationKind {
    Nop,
    Constant,
    Assign,
    Copy,
    Move,
    Call,
    Return,
    Abort,
    Assert,
    Branch,
    BranchIf,
    CompareEq,
    CompareNeq,
    CompareLt,
    CompareGt,
    CompareLe,
    CompareGe,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BorrowLocal,
    BorrowField,
    BorrowFieldMut,
    ReadField,
    WriteField,
    Pack,
    Unpack,
    CreateStruct,
    DestroyStruct,
    FreezeRef,
    BorrowGlobal,
    BorrowGlobalMut,
    MoveFrom,
    MoveTo,
    VectorOp,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: OperationId,
    pub package_id: PackageId,
    pub function_id: FunctionId,
    pub index_in_function: usize,
    pub kind: OperationKind,
    pub display: String,
    pub target: Option<String>,
    pub source_span: SourceSpan,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalInfo {
    pub id: LocalId,
    pub package_id: PackageId,
    pub function_id: FunctionId,
    pub name: String,
    pub type_name: Option<String>,
    pub index_in_function: Option<usize>,
    pub source_span: SourceSpan,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub package_id: PackageId,
    pub function_id: FunctionId,
    pub index_in_function: usize,
    pub label: String,
    pub start_operation_index: Option<usize>,
    pub end_operation_index: Option<usize>,
    pub source_span: SourceSpan,
}
