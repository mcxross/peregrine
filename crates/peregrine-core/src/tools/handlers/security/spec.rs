use codex_tools::{JsonSchema, ResponsesApiTool, ToolSpec};
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) const STATIC_RULE_CATALOG: &str = "security_sui_static_rule_catalog";
pub(crate) const STATIC_ANALYZE_PACKAGE: &str = "security_sui_static_analyze_package";
pub(crate) const PACKAGE_INSIGHTS: &str = "security_sui_package_insights";
pub(crate) const GRAPHS: &str = "security_sui_graphs";
pub(crate) const FUNCTION_STATE_GRAPH: &str = "security_sui_function_state_graph";
pub(crate) const BYTECODE_VIEW: &str = "security_sui_bytecode_view";
pub(crate) const BYTECODE_DECOMPILE: &str = "security_sui_bytecode_decompile";
pub(crate) const SUI_COMMAND: &str = "security_sui_command";
pub(crate) const MOVY_FUZZ: &str = "security_sui_movy_fuzz";
pub(crate) const FORMAL_VERIFY: &str = "security_sui_formal_verify";

pub(crate) fn create_static_rule_catalog_tool() -> ToolSpec {
    function_tool(
        STATIC_RULE_CATALOG,
        "List bundled and configured Sui Move static-analysis rules, including analyzer plugin rules when enabled.",
        common_package_properties(),
        None,
    )
}

pub(crate) fn create_static_analyze_package_tool() -> ToolSpec {
    function_tool(
        STATIC_ANALYZE_PACKAGE,
        "Run Peregrine static analysis on a Sui Move package and return findings, metrics, loaded rulesets, loaded plugins, and diagnostics.",
        common_package_properties(),
        None,
    )
}

pub(crate) fn create_package_insights_tool() -> ToolSpec {
    function_tool(
        PACKAGE_INSIGHTS,
        "Inspect a Sui Move package for security-relevant object, capability, test, formal-spec, and attack-surface signals.",
        common_package_properties(),
        None,
    )
}

pub(crate) fn create_graphs_tool() -> ToolSpec {
    function_tool(
        GRAPHS,
        "Build package-level Sui Move call, type, and state-access graphs for security review.",
        common_package_properties(),
        None,
    )
}

pub(crate) fn create_function_state_graph_tool() -> ToolSpec {
    let mut properties = common_package_properties();
    properties.insert(
        "address".to_string(),
        JsonSchema::string(Some(
            "Optional module address filter, such as `my_pkg` or `0x1`.".to_string(),
        )),
    );
    properties.insert(
        "module_name".to_string(),
        JsonSchema::string(Some(
            "Required module name containing the target function.".to_string(),
        )),
    );
    properties.insert(
        "function_name".to_string(),
        JsonSchema::string(Some(
            "Required function name for the state-access graph.".to_string(),
        )),
    );
    function_tool(
        FUNCTION_STATE_GRAPH,
        "Build a focused state-access graph for a Sui Move function.",
        properties,
        Some(vec!["module_name".to_string(), "function_name".to_string()]),
    )
}

pub(crate) fn create_bytecode_view_tool() -> ToolSpec {
    function_tool(
        BYTECODE_VIEW,
        "Load compiled Sui Move bytecode views, including functions, instructions, disassembly, and control-flow blocks. Run a build first if no bytecode exists.",
        common_package_properties(),
        None,
    )
}

pub(crate) fn create_bytecode_decompile_tool() -> ToolSpec {
    function_tool(
        BYTECODE_DECOMPILE,
        "Decompile compiled root-package Sui Move bytecode modules, with disassembly fallback when source reconstruction is incomplete.",
        common_package_properties(),
        None,
    )
}

pub(crate) fn create_sui_command_tool() -> ToolSpec {
    let mut properties = common_package_properties();
    properties.insert(
        "command_kind".to_string(),
        JsonSchema::string_enum(
            vec![
                json!("build"),
                json!("test"),
                json!("coverage"),
                json!("coverageSummary"),
                json!("moveFuzz"),
                json!("publishDryRun"),
            ],
            Some(
                "Required analysis-safe Sui command. Real `publish` is intentionally unavailable."
                    .to_string(),
            ),
        ),
    );
    properties.insert(
        "publish_build_env".to_string(),
        JsonSchema::string(Some(
            "Required only for publishDryRun: active Sui environment alias to pass as --build-env."
                .to_string(),
        )),
    );
    properties.insert(
        "with_unpublished_dependencies".to_string(),
        JsonSchema::boolean(Some(
            "For publishDryRun, include --with-unpublished-dependencies when needed.".to_string(),
        )),
    );
    properties.insert(
        "timeout_ms".to_string(),
        JsonSchema::integer(Some(
            "Optional execution timeout in milliseconds. Defaults to 120000.".to_string(),
        )),
    );
    function_tool(
        SUI_COMMAND,
        "Run an analysis-safe Sui package command through Peregrine's Sui adapter, preserving bundled vs user-installed Sui resolution and harness approvals.",
        properties,
        Some(vec!["command_kind".to_string()]),
    )
}

pub(crate) fn create_movy_fuzz_tool() -> ToolSpec {
    let mut properties = common_package_properties();
    properties.insert(
        "time_limit_seconds".to_string(),
        JsonSchema::integer(Some(
            "Optional Movy fuzz time limit in seconds. Defaults to 30.".to_string(),
        )),
    );
    properties.insert(
        "seed".to_string(),
        JsonSchema::integer(Some(
            "Optional deterministic fuzz seed. Defaults to 1.".to_string(),
        )),
    );
    function_tool(
        MOVY_FUZZ,
        "Run Movy local executor fuzzing for public Sui Move targets through the harness approval/sandbox path.",
        properties,
        None,
    )
}

pub(crate) fn create_formal_verify_tool() -> ToolSpec {
    let mut properties = common_package_properties();
    properties.insert(
        "file_path".to_string(),
        JsonSchema::string(Some(
            "Required Move source file path for the formal verification target, relative to the package or project root.".to_string(),
        )),
    );
    properties.insert(
        "module_name".to_string(),
        JsonSchema::string(Some(
            "Required module name to pass to the bundled Sui prover target filter.".to_string(),
        )),
    );
    properties.insert(
        "timeout_seconds".to_string(),
        JsonSchema::integer(Some(
            "Optional Sui prover execution timeout in seconds. Defaults to the prover adapter default.".to_string(),
        )),
    );
    function_tool(
        FORMAL_VERIFY,
        "Run bundled Sui prover formal verification for one explicit module through the harness approval/sandbox path.",
        properties,
        Some(vec!["file_path".to_string(), "module_name".to_string()]),
    )
}

fn common_package_properties() -> BTreeMap<String, JsonSchema> {
    BTreeMap::from([
        (
            "project_root".to_string(),
            JsonSchema::string(Some(
                "Optional project root. Defaults to the current turn workspace.".to_string(),
            )),
        ),
        (
            "package_path".to_string(),
            JsonSchema::string(Some(
                "Optional Move package path relative to project_root. Defaults to `.`.".to_string(),
            )),
        ),
    ])
}

fn function_tool(
    name: &str,
    description: &str,
    properties: BTreeMap<String, JsonSchema>,
    required: Option<Vec<String>>,
) -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, required, Some(false.into())),
        output_schema: None,
    })
}
