use peregrine_move_model::{MoveFunctionSignature, MoveModule, MoveStructSignature};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const CALL_GRAPH_DEPTH: usize = 5;
const STAGE_ORDER: &[&str] = &[
    "created",
    "owned",
    "mutated",
    "transferred",
    "shared",
    "wrapped",
    "immutable",
    "party",
    "deleted",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleMap {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub abilities: Vec<String>,
    pub is_capability_like: bool,
    pub stages: Vec<ObjectLifecycleStage>,
    pub touched_by: Vec<ObjectLifecycleFunctionRef>,
    pub risks: Vec<ObjectLifecycleRisk>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleStage {
    pub kind: String,
    pub functions: Vec<ObjectLifecycleFunctionRef>,
    pub evidence: Vec<String>,
}

#[derive(Serialize, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleFunctionRef {
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub direct: bool,
    pub call_path: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleRisk {
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub evidence: Vec<String>,
    pub functions: Vec<ObjectLifecycleFunctionRef>,
}

#[derive(Clone, Copy)]
struct FunctionLookup<'a> {
    module: &'a MoveModule,
    function: &'a MoveFunctionSignature,
}

struct ObjectCandidate<'a> {
    module: &'a MoveModule,
    move_struct: &'a MoveStructSignature,
    qualified_name: String,
}

#[derive(Clone)]
struct DirectEvent {
    object_key: String,
    stage: String,
    function_key: String,
    evidence: String,
}
