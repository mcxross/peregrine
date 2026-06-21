use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

mod bytecode;
mod graphs;

pub use bytecode::*;
pub use graphs::*;

pub const SERVER_NAME: &str = "peregrine-sui";
pub const SERVER_BINARY_NAME: &str = "peregrine-sui-mcp-server";
pub const SERVER_PATH_ENV: &str = "PEREGRINE_SUI_MCP_SERVER_PATH";
pub const SUI_ADAPTER_SOURCE_ENV: &str = "PEREGRINE_SUI_ADAPTER_SOURCE";
pub const SUI_CLI_PATH_ENV: &str = "PEREGRINE_SUI_CLI_PATH";
pub const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 600_000;
pub const DEFAULT_MOVY_FUZZ_TIME_LIMIT_SECONDS: u64 = 30;
pub const DEFAULT_MOVY_FUZZ_SEED: u64 = 1;
pub const DEFAULT_FORMAL_VERIFY_TIMEOUT_SECONDS: usize = 45;
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;
pub const DEFAULT_PAGE_SIZE: usize = 100;
pub const MAX_PAGE_SIZE: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiToolsConfig {
    pub mode: SuiToolsMode,
    pub adapter: SuiAdapterSettings,
}

impl Default for SuiToolsConfig {
    fn default() -> Self {
        Self {
            mode: SuiToolsMode::Auto,
            adapter: SuiAdapterSettings::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SuiToolsMode {
    #[default]
    Auto,
    Always,
    Disabled,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuiAdapterSettings {
    pub source: SuiAdapterSource,
    pub cli_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SuiAdapterSource {
    #[default]
    Bundled,
    System,
}

impl SuiAdapterSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::System => "system",
        }
    }
}

pub fn resolve_server_executable() -> PathBuf {
    let current_exe = std::env::current_exe().ok();
    resolve_server_executable_from(
        current_exe.as_deref(),
        std::env::var_os(SERVER_PATH_ENV),
        std::env::var_os("PATH"),
    )
}

pub fn resolve_server_executable_from(
    current_exe: Option<&Path>,
    injected_path: Option<OsString>,
    path: Option<OsString>,
) -> PathBuf {
    if let Some(injected_path) = injected_path {
        let injected_path = PathBuf::from(injected_path);
        if injected_path.is_file() {
            return injected_path;
        }
    }
    if let Some(current_exe) = current_exe {
        let sibling = current_exe.with_file_name(server_binary_file_name());
        if sibling.is_file() {
            return sibling;
        }
    }
    if let Some(path) = path {
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(server_binary_file_name());
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    if let Some(current_exe) = current_exe {
        return current_exe.with_file_name(server_binary_file_name());
    }
    PathBuf::from(SERVER_BINARY_NAME)
}

pub fn validate_package_name(package_name: &str) -> Result<&str, String> {
    let package_name = package_name.trim();
    if package_name.is_empty() {
        return Err("Project name cannot be empty.".to_string());
    }
    if package_name.len() > 128 {
        return Err("Project name is too long.".to_string());
    }
    let mut characters = package_name.chars();
    let Some(first) = characters.next() else {
        return Err("Project name cannot be empty.".to_string());
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err("Project name must start with a letter or underscore.".to_string());
    }
    if !characters.all(|character| character == '_' || character.is_ascii_alphanumeric()) {
        return Err("Project name can only contain letters, numbers, and underscores.".to_string());
    }
    Ok(package_name)
}

fn server_binary_file_name() -> &'static str {
    if cfg!(windows) {
        "peregrine-sui-mcp-server.exe"
    } else {
        SERVER_BINARY_NAME
    }
}

pub mod tool_name {
    pub const PACKAGE_RESOLVE: &str = "package_resolve";
    pub const MODULES: &str = "modules";
    pub const SIGNATURES: &str = "signatures";
    pub const IMPORT_PACKAGE: &str = "import_package";
    pub const CREATE_PACKAGE: &str = "create_package";
    pub const STATIC_RULE_CATALOG: &str = "static_rule_catalog";
    pub const STATIC_ANALYZE_PACKAGE: &str = "static_analyze_package";
    pub const SCANNER_REPORT: &str = "scanner_report";
    pub const TEST_SCANNER_REPORT: &str = "test_scanner_report";
    pub const PACKAGE_INSIGHTS: &str = "package_insights";
    pub const GRAPHS: &str = "graphs";
    pub const FUNCTION_STATE_GRAPH: &str = "function_state_graph";
    pub const BYTECODE_VIEW: &str = "bytecode_view";
    pub const BYTECODE_DECOMPILE: &str = "bytecode_decompile";
    pub const COMMAND: &str = "command";
    pub const MOVY_FUZZ: &str = "movy_fuzz";
    pub const FORMAL_VERIFY: &str = "formal_verify";
    pub const ANALYZE: &str = "analyze";

    pub const ALL: &[&str] = &[
        PACKAGE_RESOLVE,
        MODULES,
        SIGNATURES,
        IMPORT_PACKAGE,
        CREATE_PACKAGE,
        STATIC_RULE_CATALOG,
        STATIC_ANALYZE_PACKAGE,
        SCANNER_REPORT,
        TEST_SCANNER_REPORT,
        PACKAGE_INSIGHTS,
        GRAPHS,
        FUNCTION_STATE_GRAPH,
        BYTECODE_VIEW,
        BYTECODE_DECOMPILE,
        COMMAND,
        MOVY_FUZZ,
        FORMAL_VERIFY,
        ANALYZE,
    ];
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PackageArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub package_path: Option<String>,
    #[serde(default)]
    pub unbounded: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectGraphsArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub include_external: bool,
    #[serde(default)]
    pub depth: Option<usize>,
    #[serde(default)]
    pub response_format: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BytecodeViewArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub include_external: bool,
    #[serde(default)]
    pub response_format: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignaturesArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModulesArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImportPackageArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    pub network_id: String,
    pub graph_ql_url: String,
    pub package_id: String,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub raw_only: bool,
    #[serde(default)]
    pub max_dependency_depth: Option<usize>,
    #[serde(default)]
    pub max_dependency_packages: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreatePackageArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    pub package_name: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StaticAnalysisArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub no_global_plugins: bool,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub rulesets: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TestScannerArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    pub source_mode: ScannerSourceMode,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScannerSourceMode {
    BestAvailable,
    SourceOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FunctionStateGraphArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub address: Option<String>,
    pub module_name: String,
    pub function_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuiCommandArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    pub command_kind: String,
    #[serde(default)]
    pub publish_build_env: Option<String>,
    #[serde(default)]
    pub with_unpublished_dependencies: Option<bool>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MovyFuzzArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    #[serde(default)]
    pub time_limit_seconds: Option<u64>,
    #[serde(default)]
    pub seed: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FormalVerifyArgs {
    #[serde(flatten)]
    pub package: PackageArgs,
    pub file_path: String,
    pub module_name: String,
    #[serde(default)]
    pub timeout_seconds: Option<usize>,
    #[serde(default)]
    pub trace: bool,
    #[serde(default)]
    pub keep_temp: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum AnalyzeTargetArgs {
    #[serde(rename = "localPackage")]
    LocalPackage {
        #[serde(default)]
        project_root: Option<String>,
        #[serde(default)]
        package_path: Option<String>,
    },
    #[serde(rename = "onChainPackage")]
    OnChainPackage {
        #[serde(default)]
        project_root: Option<String>,
        network_id: String,
        graph_ql_url: String,
        package_id: String,
        #[serde(default)]
        max_dependency_depth: Option<usize>,
        #[serde(default)]
        max_dependency_packages: Option<usize>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnalyzeArgs {
    pub target: AnalyzeTargetArgs,
    #[serde(default)]
    pub stages: Vec<peregrine_analysis::AnalysisStage>,
    #[serde(default)]
    pub graph_kinds: Vec<peregrine_analysis::GraphKind>,
    #[serde(default)]
    pub plugin_ids: Vec<String>,
    #[serde(default)]
    pub dynamic_capabilities: Vec<String>,
    #[serde(default)]
    pub limits: Option<peregrine_analysis::AnalysisLimits>,
    #[serde(default)]
    pub options: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineAnalysisResponse {
    pub status: String,
    pub report: peregrine_analysis::AnalysisReport,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub project_root: String,
    pub package_root: String,
    pub package_path: String,
    pub package_name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub status: CommandStatus,
    pub package: PackageSummary,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandStatus {
    Completed,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureEntry {
    pub module_name: String,
    pub module_address: Option<String>,
    pub file_path: String,
    pub function_name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleEntry {
    pub module_name: String,
    pub module_address: Option<String>,
    pub file_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSourceSummary {
    pub manifest_path: String,
    pub source_file_count: usize,
    pub has_source_modules: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModulesPage {
    pub status: String,
    pub package: PackageSummary,
    pub source: MoveSourceSummary,
    pub data: Vec<ModuleEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignaturesPage {
    pub status: String,
    pub package: PackageSummary,
    pub data: Vec<SignatureEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPackageResponse {
    pub status: String,
    pub import_root: String,
    pub artifact: ImportArtifact,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportArtifact {
    pub raw_root: String,
    pub buildable_root: String,
    pub project_root: String,
    pub root_package_id: String,
    pub root_package_name: String,
    pub dependencies: Vec<ImportDependency>,
    pub diagnostics: Vec<ImportDiagnostic>,
    pub build_result: Option<ImportBuildResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDependency {
    pub package_id: String,
    pub package_name: String,
    pub depth: usize,
    pub local_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDiagnostic {
    pub severity: ImportDiagnosticSeverity,
    pub stage: String,
    pub package_id: Option<String>,
    pub module: Option<String>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBuildResult {
    pub command: String,
    pub success: bool,
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticAnalysisResponse {
    pub status: String,
    pub package: PackageSummary,
    pub report: AnalysisReport,
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub findings: Vec<AnalysisFinding>,
    pub metrics: Vec<AnalysisRuleMetric>,
    pub loaded_rulesets: Vec<String>,
    pub loaded_plugins: Vec<String>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisFinding {
    pub rule_id: String,
    pub ruleset_id: String,
    pub severity: AnalysisSeverity,
    pub message: String,
    pub file: String,
    pub span: Option<AnalysisSpan>,
    pub metric: Option<AnalysisMetric>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRuleMetric {
    pub ruleset_id: String,
    pub rule_id: String,
    pub target: String,
    pub file: Option<String>,
    pub span: Option<AnalysisSpan>,
    pub metric: AnalysisMetric,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisMetric {
    pub name: String,
    pub value: u32,
    pub threshold: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSpan {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisDiagnostic {
    pub level: String,
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticRuleCatalogResponse {
    pub status: String,
    pub package: PackageSummary,
    pub catalog: AnalysisRuleCatalog,
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRuleCatalog {
    pub rulesets: Vec<AnalysisRuleSet>,
    pub loaded_plugins: Vec<String>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRuleSet {
    pub id: String,
    pub name: String,
    pub description: String,
    pub bundled: bool,
    pub plugin_id: Option<String>,
    pub active: bool,
    pub rules: Vec<AnalysisRule>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub default_severity: AnalysisSeverity,
    pub configured_severity: Option<AnalysisSeverity>,
    pub config_schema: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestScannerResponse {
    pub status: String,
    pub package: PackageSummary,
    pub report: TestScannerReport,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestScannerReport {
    pub unit_test_count: usize,
    pub movy_invariant_test_count: usize,
    pub random_test_count: usize,
    pub formal_prover_spec_count: usize,
    pub diagnostics: Vec<ScannerDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerDiagnostic {
    pub severity: ScannerDiagnosticSeverity,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScannerDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

pub fn tool_definitions() -> Vec<ToolDefinition> {
    use tool_name::*;

    vec![
        read_tool(
            PACKAGE_RESOLVE,
            "Resolve and validate a Sui Move package inside the MCP workspace.",
            package_schema(),
        ),
        read_tool(
            MODULES,
            "List Sui Move modules using bounded cursor pagination.",
            module_inventory_schema(),
        ),
        read_tool(
            SIGNATURES,
            "List Sui Move function signatures using bounded cursor pagination.",
            module_inventory_schema(),
        ),
        ToolDefinition {
            name: IMPORT_PACKAGE,
            description: "Import an on-chain Sui package into a bounded workspace-contained artifact.",
            input_schema: object_schema(
                vec![
                    package_project_root(),
                    string_property("network_id", "Sui network identifier."),
                    string_property("graph_ql_url", "Sui GraphQL endpoint."),
                    string_property("package_id", "On-chain Sui package id."),
                    string_property("output_path", "Output path relative to project_root."),
                    ("raw_only", json!({"type": "boolean"})),
                    (
                        "max_dependency_depth",
                        json!({"type": "integer", "minimum": 0, "maximum": 16}),
                    ),
                    (
                        "max_dependency_packages",
                        json!({"type": "integer", "minimum": 1, "maximum": 512}),
                    ),
                ],
                &["network_id", "graph_ql_url", "package_id"],
            ),
            read_only: false,
            destructive: true,
            open_world: true,
        },
        ToolDefinition {
            name: CREATE_PACKAGE,
            description: "Create a new local Sui Move package inside the MCP workspace.",
            input_schema: object_schema(
                vec![
                    package_project_root(),
                    string_property("package_name", "New Move package name."),
                    ("timeout_ms", json!({"type": "integer", "minimum": 1})),
                ],
                &["package_name"],
            ),
            read_only: false,
            destructive: true,
            open_world: false,
        },
        read_tool(
            STATIC_RULE_CATALOG,
            "List bundled and configured Sui Move static-analysis rules.",
            static_analysis_schema(),
        ),
        read_tool(
            STATIC_ANALYZE_PACKAGE,
            "Run Peregrine static analysis on a Sui Move package. Audit capability: static.analysis.",
            static_analysis_schema(),
        ),
        read_tool(
            SCANNER_REPORT,
            "Run Peregrine Sui Move scanners and return source-backed evidence.",
            package_schema(),
        ),
        read_tool(
            TEST_SCANNER_REPORT,
            "Inspect Sui Move unit, fuzz, invariant, and formal-verification tests.",
            object_schema(
                vec![
                    package_project_root(),
                    package_path(),
                    (
                        "source_mode",
                        json!({
                            "type": "string",
                            "enum": ["bestAvailable", "sourceOnly"],
                        }),
                    ),
                ],
                &["source_mode"],
            ),
        ),
        read_tool(
            PACKAGE_INSIGHTS,
            "Inspect a Sui Move package for security-relevant signals.",
            package_schema(),
        ),
        read_tool(
            GRAPHS,
            "Build package-level Sui Move call, type, and state-access graphs. Audit capability: graph.analysis.",
            package_schema(),
        ),
        read_tool(
            FUNCTION_STATE_GRAPH,
            "Build a focused state-access graph for a Sui Move function. Audit capability: graph.analysis.",
            object_schema(
                vec![
                    package_project_root(),
                    package_path(),
                    string_property("address", "Optional module address filter."),
                    string_property("module_name", "Module containing the target function."),
                    string_property("function_name", "Target function name."),
                ],
                &["module_name", "function_name"],
            ),
        ),
        read_tool(
            BYTECODE_VIEW,
            "Load compiled Sui Move bytecode, disassembly, and control-flow blocks. Audit capability: bytecode.analysis.",
            package_schema(),
        ),
        read_tool(
            BYTECODE_DECOMPILE,
            "Decompile compiled root-package Sui Move bytecode modules. Audit capability: bytecode.analysis.",
            package_schema(),
        ),
        ToolDefinition {
            name: COMMAND,
            description: "Run an analysis-safe Sui package command. Real publish is unavailable.",
            input_schema: object_schema(
                vec![
                    package_project_root(),
                    package_path(),
                    (
                        "command_kind",
                        json!({
                            "type": "string",
                            "enum": [
                                "build",
                                "test",
                                "coverage",
                                "coverageSummary",
                                "moveFuzz",
                                "publishDryRun"
                            ]
                        }),
                    ),
                    string_property(
                        "publish_build_env",
                        "Active Sui environment alias for publishDryRun.",
                    ),
                    ("with_unpublished_dependencies", json!({"type": "boolean"})),
                    ("timeout_ms", json!({"type": "integer", "minimum": 1})),
                ],
                &["command_kind"],
            ),
            read_only: false,
            destructive: true,
            open_world: true,
        },
        ToolDefinition {
            name: MOVY_FUZZ,
            description: "Run Movy local executor fuzzing for public Sui Move targets. Audit capability: dynamic.fuzzing.",
            input_schema: object_schema(
                vec![
                    package_project_root(),
                    package_path(),
                    (
                        "time_limit_seconds",
                        json!({"type": "integer", "minimum": 1}),
                    ),
                    ("seed", json!({"type": "integer", "minimum": 0})),
                ],
                &[],
            ),
            read_only: false,
            destructive: true,
            open_world: false,
        },
        ToolDefinition {
            name: FORMAL_VERIFY,
            description: "Run bundled Sui prover formal verification for one module. Audit capability: formal.verification.",
            input_schema: object_schema(
                vec![
                    package_project_root(),
                    package_path(),
                    string_property("file_path", "Move source file for verification."),
                    string_property("module_name", "Module passed to the prover target filter."),
                    ("timeout_seconds", json!({"type": "integer", "minimum": 1})),
                    ("trace", json!({"type": "boolean"})),
                    ("keep_temp", json!({"type": "boolean"})),
                ],
                &["file_path", "module_name"],
            ),
            read_only: false,
            destructive: true,
            open_world: false,
        },
        ToolDefinition {
            name: ANALYZE,
            description: "Run the shared Sui analysis engine over a local or on-chain package. Audit capabilities: static.analysis, graph.analysis, bytecode.analysis, dynamic.fuzzing, formal.verification.",
            input_schema: analyze_schema(),
            read_only: false,
            destructive: false,
            open_world: true,
        },
    ]
}

fn read_tool(name: &'static str, description: &'static str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name,
        description,
        input_schema,
        read_only: true,
        destructive: false,
        open_world: false,
    }
}

fn package_schema() -> Value {
    object_schema(vec![package_project_root(), package_path()], &[])
}

fn static_analysis_schema() -> Value {
    object_schema(
        vec![
            package_project_root(),
            package_path(),
            ("no_global_plugins", json!({"type": "boolean"})),
            (
                "plugins",
                json!({
                    "type": "array",
                    "items": {"type": "string"},
                }),
            ),
            (
                "rulesets",
                json!({
                    "type": "array",
                    "items": {"type": "string"},
                }),
            ),
        ],
        &[],
    )
}

fn analyze_schema() -> Value {
    object_schema(
        vec![
            (
                "target",
                json!({
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {
                                "kind": {"const": "localPackage"},
                                "project_root": {"type": "string"},
                                "package_path": {"type": "string"}
                            },
                            "required": ["kind"],
                            "additionalProperties": false
                        },
                        {
                            "type": "object",
                            "properties": {
                                "kind": {"const": "onChainPackage"},
                                "project_root": {"type": "string"},
                                "network_id": {"type": "string"},
                                "graph_ql_url": {"type": "string"},
                                "package_id": {"type": "string"},
                                "max_dependency_depth": {
                                    "type": "integer",
                                    "minimum": 0,
                                    "maximum": 16
                                },
                                "max_dependency_packages": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "maximum": 512
                                }
                            },
                            "required": ["kind", "network_id", "graph_ql_url", "package_id"],
                            "additionalProperties": false
                        }
                    ]
                }),
            ),
            (
                "stages",
                json!({
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["scan", "graph", "static", "dynamic"]
                    }
                }),
            ),
            (
                "graph_kinds",
                json!({
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": [
                            "dependency",
                            "call",
                            "controlFlow",
                            "dataFlow",
                            "type",
                            "stateAccess"
                        ]
                    }
                }),
            ),
            (
                "plugin_ids",
                json!({"type": "array", "items": {"type": "string"}}),
            ),
            (
                "dynamic_capabilities",
                json!({
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": [
                            "fuzzing",
                            "formalVerification",
                            "simulation",
                            "symbolicExecution",
                            "runtimeAnalysis"
                        ]
                    }
                }),
            ),
            ("limits", json!({"type": "object"})),
            ("options", json!({"type": "object"})),
        ],
        &["target"],
    )
}

fn module_inventory_schema() -> Value {
    object_schema(
        vec![
            package_project_root(),
            package_path(),
            (
                "modules",
                json!({
                    "type": "array",
                    "items": {"type": "string"},
                }),
            ),
            string_property("file", "Project-relative Move source file."),
            string_property("cursor", "Opaque cursor returned by the previous page."),
            (
                "limit",
                json!({
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_PAGE_SIZE,
                }),
            ),
        ],
        &[],
    )
}

fn package_project_root() -> (&'static str, Value) {
    string_property(
        "project_root",
        "Project root relative to the MCP server workspace.",
    )
}

fn package_path() -> (&'static str, Value) {
    string_property(
        "package_path",
        "Move package path relative to project_root.",
    )
}

fn string_property(name: &'static str, description: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({
            "type": "string",
            "description": description,
        }),
    )
}

fn object_schema(properties: Vec<(&'static str, Value)>, required: &[&str]) -> Value {
    let properties = properties
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
        .collect::<serde_json::Map<_, _>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    #[test]
    fn inventory_defines_every_public_tool_once() {
        let expected = [
            "package_resolve",
            "modules",
            "signatures",
            "import_package",
            "create_package",
            "static_rule_catalog",
            "static_analyze_package",
            "scanner_report",
            "test_scanner_report",
            "package_insights",
            "graphs",
            "function_state_graph",
            "bytecode_view",
            "bytecode_decompile",
            "command",
            "movy_fuzz",
            "formal_verify",
            "analyze",
        ];
        let definitions = tool_definitions();
        let unique_names = definitions
            .iter()
            .map(|definition| definition.name)
            .collect::<BTreeSet<_>>();
        let names = definitions
            .iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert_eq!(unique_names.len(), definitions.len());
        assert_eq!(tool_name::ALL, expected);
        assert_eq!(names, expected);
        assert!(names.iter().all(|name| !name.starts_with("security_sui_")));
    }

    #[test]
    fn command_surface_does_not_expose_real_publish() {
        let Some(definition) = tool_definitions()
            .into_iter()
            .find(|definition| definition.name == tool_name::COMMAND)
        else {
            panic!("Sui command definition is missing");
        };
        let command_kind = &definition.input_schema["properties"]["command_kind"]["enum"];
        let Some(command_kinds) = command_kind.as_array() else {
            panic!("command kind enum is not an array");
        };

        assert!(!command_kinds.contains(&json!("publish")));
    }

    #[test]
    fn audit_capability_phrases_are_searchable() {
        let descriptions = tool_definitions()
            .into_iter()
            .map(|definition| (definition.name, definition.description))
            .collect::<BTreeMap<_, _>>();

        for (tool, capability) in [
            (tool_name::STATIC_ANALYZE_PACKAGE, "static.analysis"),
            (tool_name::GRAPHS, "graph.analysis"),
            (tool_name::FUNCTION_STATE_GRAPH, "graph.analysis"),
            (tool_name::BYTECODE_VIEW, "bytecode.analysis"),
            (tool_name::BYTECODE_DECOMPILE, "bytecode.analysis"),
            (tool_name::MOVY_FUZZ, "dynamic.fuzzing"),
            (tool_name::FORMAL_VERIFY, "formal.verification"),
        ] {
            assert!(
                descriptions
                    .get(tool)
                    .is_some_and(|description| description.contains(capability)),
                "{tool} description should contain {capability}",
            );
        }
    }

    #[test]
    fn package_names_are_validated_at_the_protocol_boundary() {
        assert_eq!(validate_package_name("vault"), Ok("vault"));
        assert!(validate_package_name("../vault").is_err());
    }

    #[test]
    fn executable_resolution_prefers_injected_then_sibling_then_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let current = temp.path().join("bin/peregrine");
        let sibling = current.with_file_name(server_binary_file_name());
        let injected = temp.path().join("injected/server");
        let path_server = temp.path().join("path").join(server_binary_file_name());
        for file in [&current, &sibling, &injected, &path_server] {
            let Some(parent) = file.parent() else {
                panic!("test executable path has no parent");
            };
            fs::create_dir_all(parent)?;
            fs::write(file, "")?;
        }
        let Some(path_parent) = path_server.parent() else {
            panic!("test PATH executable has no parent");
        };
        let path = std::env::join_paths([path_parent])?;

        assert_eq!(
            resolve_server_executable_from(
                Some(&current),
                Some(injected.clone().into_os_string()),
                Some(path.clone()),
            ),
            injected
        );
        assert_eq!(
            resolve_server_executable_from(Some(&current), None, Some(path.clone())),
            sibling
        );
        fs::remove_file(&sibling)?;
        assert_eq!(
            resolve_server_executable_from(Some(&current), None, Some(path)),
            path_server
        );
        fs::remove_file(&path_server)?;
        assert_eq!(
            resolve_server_executable_from(Some(&current), None, None),
            sibling
        );
        Ok(())
    }
}
