use crate::{
    output::{CliDiagnostic, CliStep},
    session::McpToolClient,
    sui::{args::FuzzArgs, project::CliContext, runners::process::mcp_command_step},
};
use peregrine_sui_mcp_protocol::{MovyFuzzArgs, PackageArgs, tool_name};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

pub fn run_fuzz(context: &CliContext, args: &FuzzArgs) -> CliStep {
    let started_at = Instant::now();
    let result = McpToolClient::call_blocking::<_, peregrine_sui_mcp_protocol::CommandResult>(
        &context.project_root,
        tool_name::MOVY_FUZZ,
        &MovyFuzzArgs {
            package: PackageArgs {
                project_root: None,
                package_path: Some(context.package_path.clone()),
                unbounded: false,
            },
            time_limit_seconds: Some(args.time_limit_seconds),
            seed: Some(args.seed),
        },
    );

    match result {
        Ok(result) => mcp_command_step(
            "fuzz",
            started_at,
            result,
            BTreeMap::from([
                (
                    "engine".to_string(),
                    Value::String("movy-local-executor".to_string()),
                ),
                ("seed".to_string(), json!(args.seed)),
                (
                    "timeLimitSeconds".to_string(),
                    json!(args.time_limit_seconds),
                ),
            ]),
        ),
        Err(error) => CliStep::failed(
            "fuzz",
            started_at,
            CliDiagnostic::error("mcp:peregrine", error),
        ),
    }
}
