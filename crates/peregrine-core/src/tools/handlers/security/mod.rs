mod command_tools;
mod read_tools;
mod spec;

pub(crate) use command_tools::{
    FormalVerifyHandler as SecurityFormalVerifyHandler, MovyFuzzHandler as SecurityMovyFuzzHandler,
    SuiCommandHandler as SecuritySuiCommandHandler,
};
pub(crate) use read_tools::{ReadToolHandler, ReadToolKind as SecurityReadToolKind};

const TOOL_SEARCH_SOURCE_NAME: &str = "Sui security tools";
const TOOL_SEARCH_SOURCE_DESCRIPTION: &str =
    "Static, dynamic, bytecode, graph, and adapter-backed Sui Move security analysis tools.";

#[cfg(test)]
pub(crate) const SECURITY_TOOL_NAMES: &[&str] = &[
    spec::STATIC_RULE_CATALOG,
    spec::STATIC_ANALYZE_PACKAGE,
    spec::PACKAGE_INSIGHTS,
    spec::GRAPHS,
    spec::FUNCTION_STATE_GRAPH,
    spec::BYTECODE_VIEW,
    spec::BYTECODE_DECOMPILE,
    spec::SUI_COMMAND,
    spec::MOVY_FUZZ,
    spec::FORMAL_VERIFY,
];

fn tool_search_info(
    spec: codex_tools::ToolSpec,
    search_text: String,
) -> Option<crate::tools::tool_search_entry::ToolSearchInfo> {
    crate::tools::tool_search_entry::ToolSearchInfo::from_spec(
        search_text,
        spec,
        Some(codex_tools::ToolSearchSourceInfo {
            name: TOOL_SEARCH_SOURCE_NAME.to_string(),
            description: Some(TOOL_SEARCH_SOURCE_DESCRIPTION.to_string()),
        }),
    )
}
