use crate::function_tool::FunctionCallError;
use crate::tools::context::{
    FunctionToolOutput, ToolInvocation, ToolOutput, ToolPayload, boxed_tool_output,
};
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::{CoreToolRuntime, ToolExecutor};
use crate::tools::tool_search_entry::ToolSearchInfo;
use codex_tools::{ToolName, ToolSpec};
use peregrine_security_tools::{
    SecurityToolsError, resolve_move_package, static_analyze_package, static_rule_catalog,
    sui_bytecode_decompile, sui_bytecode_view, sui_function_state_graph, sui_graphs,
    sui_package_insights,
};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;

use super::{spec, tool_search_info};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadToolKind {
    StaticRuleCatalog,
    StaticAnalyzePackage,
    PackageInsights,
    Graphs,
    FunctionStateGraph,
    BytecodeView,
    BytecodeDecompile,
}

pub(crate) struct ReadToolHandler {
    kind: ReadToolKind,
}

impl ReadToolHandler {
    pub(crate) fn new(kind: ReadToolKind) -> Self {
        Self { kind }
    }

    fn search_text(&self) -> String {
        match self.kind {
            ReadToolKind::StaticRuleCatalog => "sui move static analysis rules rule catalog analyzer plugins security".to_string(),
            ReadToolKind::StaticAnalyzePackage => "sui move static analysis analyze package vulnerabilities findings metrics".to_string(),
            ReadToolKind::PackageInsights => "sui move package insights attack surface capabilities shared objects tests formal specs".to_string(),
            ReadToolKind::Graphs => "sui move call graph type graph state access graph package security".to_string(),
            ReadToolKind::FunctionStateGraph => "sui move function state access graph object reads writes borrows security".to_string(),
            ReadToolKind::BytecodeView => "sui move bytecode view disassembly instructions control flow cfg compiled package".to_string(),
            ReadToolKind::BytecodeDecompile => "sui move bytecode decompile disassembly source reconstruction reverse engineering".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ReadToolHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(match self.kind {
            ReadToolKind::StaticRuleCatalog => spec::STATIC_RULE_CATALOG,
            ReadToolKind::StaticAnalyzePackage => spec::STATIC_ANALYZE_PACKAGE,
            ReadToolKind::PackageInsights => spec::PACKAGE_INSIGHTS,
            ReadToolKind::Graphs => spec::GRAPHS,
            ReadToolKind::FunctionStateGraph => spec::FUNCTION_STATE_GRAPH,
            ReadToolKind::BytecodeView => spec::BYTECODE_VIEW,
            ReadToolKind::BytecodeDecompile => spec::BYTECODE_DECOMPILE,
        })
    }

    fn spec(&self) -> ToolSpec {
        match self.kind {
            ReadToolKind::StaticRuleCatalog => spec::create_static_rule_catalog_tool(),
            ReadToolKind::StaticAnalyzePackage => spec::create_static_analyze_package_tool(),
            ReadToolKind::PackageInsights => spec::create_package_insights_tool(),
            ReadToolKind::Graphs => spec::create_graphs_tool(),
            ReadToolKind::FunctionStateGraph => spec::create_function_state_graph_tool(),
            ReadToolKind::BytecodeView => spec::create_bytecode_view_tool(),
            ReadToolKind::BytecodeDecompile => spec::create_bytecode_decompile_tool(),
        }
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let ToolInvocation { turn, payload, .. } = invocation;
        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "{} handler received unsupported payload",
                self.tool_name()
            )));
        };

        let args: ReadToolArgs = parse_arguments(&arguments)?;
        let ctx = resolve_context(turn.as_ref(), &args)?;
        let package = PackageSummary::from_context(&ctx);
        let value = match self.kind {
            ReadToolKind::StaticRuleCatalog => json!({
                "status": "ok",
                "package": package,
                "catalog": static_rule_catalog(&ctx),
            }),
            ReadToolKind::StaticAnalyzePackage => json!({
                "status": "ok",
                "package": package,
                "report": static_analyze_package(&ctx),
            }),
            ReadToolKind::PackageInsights => json!({
                "status": "ok",
                "package": package,
                "report": sui_package_insights(&ctx).map_err(to_model_error)?,
            }),
            ReadToolKind::Graphs => json!({
                "status": "ok",
                "package": package,
                "graphs": sui_graphs(&ctx),
            }),
            ReadToolKind::FunctionStateGraph => {
                let module_name = args.module_name.as_deref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "module_name is required for security_sui_function_state_graph".to_string(),
                    )
                })?;
                let function_name = args.function_name.as_deref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "function_name is required for security_sui_function_state_graph"
                            .to_string(),
                    )
                })?;
                json!({
                    "status": "ok",
                    "package": package,
                    "graph": sui_function_state_graph(
                        &ctx,
                        args.address,
                        module_name,
                        function_name
                    ),
                })
            }
            ReadToolKind::BytecodeView => json!({
                "status": "ok",
                "package": package,
                "bytecode": sui_bytecode_view(&ctx).map_err(to_model_error)?,
            }),
            ReadToolKind::BytecodeDecompile => json!({
                "status": "ok",
                "package": package,
                "decompiled": sui_bytecode_decompile(&ctx).map_err(to_model_error)?,
            }),
        };

        Ok(boxed_tool_output(json_output(value)?))
    }
}

impl CoreToolRuntime for ReadToolHandler {
    fn search_info(&self) -> Option<ToolSearchInfo> {
        tool_search_info(self.spec(), self.search_text())
    }
}

#[derive(Debug, Deserialize)]
struct ReadToolArgs {
    #[serde(default)]
    project_root: Option<String>,
    #[serde(default)]
    package_path: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    module_name: Option<String>,
    #[serde(default)]
    function_name: Option<String>,
}

fn resolve_context(
    turn: &crate::session::turn_context::TurnContext,
    args: &ReadToolArgs,
) -> Result<peregrine_security_tools::MovePackageContext, FunctionCallError> {
    let project_root = resolve_project_root(turn, args.project_root.as_deref())?;
    resolve_move_package(project_root, args.package_path.as_deref()).map_err(to_model_error)
}

pub(crate) fn resolve_project_root(
    turn: &crate::session::turn_context::TurnContext,
    project_root: Option<&str>,
) -> Result<PathBuf, FunctionCallError> {
    let workspace_root = turn
        .environments
        .primary()
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "Sui security tools require a turn workspace environment".to_string(),
            )
        })?
        .cwd
        .to_path_buf();
    let workspace_root = std::fs::canonicalize(&workspace_root).map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "failed to resolve turn workspace {}: {error}",
            workspace_root.display()
        ))
    })?;
    let candidate = project_root
        .filter(|value| !value.trim().is_empty())
        .map_or_else(
            || workspace_root.clone(),
            |value| workspace_root.join(value),
        );
    let candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "failed to resolve project_root {}: {error}",
            candidate.display()
        ))
    })?;
    if !candidate.starts_with(&workspace_root) {
        return Err(FunctionCallError::RespondToModel(format!(
            "project_root must remain inside the turn workspace {}",
            workspace_root.display()
        )));
    }
    Ok(candidate)
}

pub(crate) fn to_model_error(error: SecurityToolsError) -> FunctionCallError {
    FunctionCallError::RespondToModel(error.to_string())
}

pub(crate) fn json_output(
    value: serde_json::Value,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let text = serde_json::to_string_pretty(&value).map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "failed to serialize security tool output: {error}"
        ))
    })?;
    Ok(FunctionToolOutput::from_text(text, Some(true)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageSummary {
    project_root: String,
    package_root: String,
    package_path: String,
    package_name: String,
}

impl PackageSummary {
    fn from_context(ctx: &peregrine_security_tools::MovePackageContext) -> Self {
        Self {
            project_root: ctx.project_root.display().to_string(),
            package_root: ctx.package_root.display().to_string(),
            package_path: ctx.package_path.clone(),
            package_name: ctx.package_name.clone(),
        }
    }
}
