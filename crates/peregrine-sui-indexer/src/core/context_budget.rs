use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextLevel {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextBudget {
    pub max_tokens_estimate: usize,
    pub level: ContextLevel,
    pub max_operations: usize,
    pub max_call_depth: usize,
    pub max_callees: usize,
    pub max_callers: usize,
    pub max_related_types: usize,
    pub max_source_excerpt_lines: usize,
    pub include_source: bool,
    pub include_full_source: bool,
    pub include_operation_raw_json: bool,
    pub include_raw_summary_json: bool,
    pub include_diagnostics: bool,
    pub include_semantic_tags: bool,
    pub include_related_types: bool,
    pub include_callers: bool,
    pub include_callees: bool,
    pub include_reachable_graph: bool,
    pub materialize_dependency_summaries: bool,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_tokens_estimate: 500,
            level: ContextLevel::Level1,
            max_operations: 25,
            max_call_depth: 1,
            max_callees: 8,
            max_callers: 4,
            max_related_types: 6,
            max_source_excerpt_lines: 24,
            include_source: false,
            include_full_source: false,
            include_operation_raw_json: false,
            include_raw_summary_json: false,
            include_diagnostics: true,
            include_semantic_tags: true,
            include_related_types: true,
            include_callers: false,
            include_callees: true,
            include_reachable_graph: false,
            materialize_dependency_summaries: false,
        }
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}
