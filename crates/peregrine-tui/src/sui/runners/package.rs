use crate::{
    output::{CliDiagnostic, CliStatus, CliStep},
    session::McpToolClient,
    sui::{project::CliContext, runners::process::mcp_command_step},
};
use peregrine_sui_mcp_protocol::{PackageArgs, SuiCommandArgs, tool_name};
use serde_json::Value;
use std::{collections::BTreeMap, time::Instant};

pub fn run_build(context: &CliContext) -> CliStep {
    run_sui_step(context, "build", "build")
}

pub fn run_test(context: &CliContext) -> CliStep {
    run_sui_step(context, "test", "test")
}

pub fn run_coverage(context: &CliContext) -> Vec<CliStep> {
    let coverage = run_sui_step(context, "coverage", "coverage");

    if coverage.status != CliStatus::Passed {
        return vec![coverage];
    }

    let summary = run_sui_step(context, "coverage-summary", "coverageSummary");
    vec![coverage, summary]
}

fn run_sui_step(context: &CliContext, name: &str, command_kind: &str) -> CliStep {
    let started_at = Instant::now();
    let result = McpToolClient::call_blocking::<_, peregrine_sui_mcp_protocol::CommandResult>(
        &context.project_root,
        tool_name::COMMAND,
        &SuiCommandArgs {
            package: PackageArgs {
                project_root: None,
                package_path: Some(context.package_path.clone()),
            },
            command_kind: command_kind.to_string(),
            publish_build_env: None,
            with_unpublished_dependencies: None,
            timeout_ms: None,
        },
    );

    match result {
        Ok(result) => mcp_command_step(
            name,
            started_at,
            result,
            BTreeMap::from([(
                "execution".to_string(),
                Value::String("mcp:peregrine".to_string()),
            )]),
        ),
        Err(error) => CliStep::failed(
            name,
            started_at,
            CliDiagnostic::error("mcp:peregrine", error),
        ),
    }
}
