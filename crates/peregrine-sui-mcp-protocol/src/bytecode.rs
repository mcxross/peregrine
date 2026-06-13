use crate::PackageSummary;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodePackageView {
    pub package_name: String,
    pub package_path: String,
    pub build_path: String,
    pub module_count: usize,
    pub function_count: usize,
    pub instruction_count: usize,
    pub struct_count: usize,
    pub constant_count: usize,
    pub dependency_count: usize,
    pub source_map_count: usize,
    pub modules: Vec<MoveBytecodeModuleView>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeModuleView {
    pub name: String,
    pub address: String,
    pub package_name: String,
    pub is_dependency: bool,
    pub bytecode_path: String,
    pub source_map_path: Option<String>,
    pub source_path: Option<String>,
    pub byte_size: u64,
    pub version: u32,
    pub function_count: usize,
    pub instruction_count: usize,
    pub struct_count: usize,
    pub constant_count: usize,
    pub import_count: usize,
    pub friend_count: usize,
    pub functions: Vec<MoveBytecodeFunctionView>,
    pub imports: Vec<String>,
    pub disassembly: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeFunctionView {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub parameters: Vec<String>,
    pub returns: Vec<String>,
    pub type_parameter_count: usize,
    pub instruction_count: usize,
    pub local_count: usize,
    pub return_count: usize,
    pub acquires: Vec<String>,
    pub instructions: Vec<MoveBytecodeInstructionView>,
    pub control_flow: MoveBytecodeControlFlowView,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeInstructionView {
    pub offset: u16,
    pub opcode: String,
    pub detail: String,
    pub call: Option<MoveBytecodeCallView>,
    pub source: Option<MoveBytecodeSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeCallView {
    pub handle_index: u16,
    pub module_address: String,
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub generic_type_arguments: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeControlFlowView {
    pub blocks: Vec<MoveBytecodeBasicBlockView>,
    pub edges: Vec<MoveBytecodeControlFlowEdgeView>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeBasicBlockView {
    pub id: String,
    pub label: String,
    pub start_offset: u16,
    pub end_offset: u16,
    pub instruction_offsets: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeControlFlowEdgeView {
    pub source: String,
    pub target: String,
    pub source_offset: u16,
    pub target_offset: u16,
    pub kind: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBytecodeSourceSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BytecodeViewResponse {
    pub status: String,
    pub package: PackageSummary,
    pub bytecode: MoveBytecodePackageView,
}
