use crate::{
    output::{CliDiagnostic, CliStep},
    session::McpToolClient,
    sui::{
        args::VerifyArgs,
        project::{CliContext, FormalTarget, formal_targets},
        runners::process::mcp_command_step,
    },
};
use peregrine_mcp_protocol::{FormalVerifyArgs, PackageArgs, tool_name};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

pub fn run_verify(context: &CliContext, args: &VerifyArgs) -> Vec<CliStep> {
    let targets = match formal_targets(context, args) {
        Ok(targets) => targets,
        Err(error) => return vec![CliStep::failed("verify", Instant::now(), error)],
    };

    targets
        .into_iter()
        .map(|target| run_verify_target(context, args, target))
        .collect()
}

fn run_verify_target(context: &CliContext, args: &VerifyArgs, target: FormalTarget) -> CliStep {
    let started_at = Instant::now();
    let result = McpToolClient::call_blocking::<_, peregrine_mcp_protocol::CommandResult>(
        &context.project_root,
        tool_name::FORMAL_VERIFY,
        &FormalVerifyArgs {
            package: PackageArgs {
                project_root: None,
                package_path: Some(context.package_path.clone()),
            },
            file_path: target.file_path.clone(),
            module_name: target.module_name.clone(),
            timeout_seconds: Some(args.timeout_seconds),
            trace: args.trace,
            keep_temp: args.keep_temp,
        },
    );

    match result {
        Ok(result) => mcp_command_step(
            format!("verify:{}", target.module_name),
            started_at,
            result,
            BTreeMap::from([
                (
                    "packageRoot".to_string(),
                    Value::String(context.package_root.display().to_string()),
                ),
                ("file".to_string(), Value::String(target.file_path)),
                ("module".to_string(), Value::String(target.module_name)),
                ("timeoutSeconds".to_string(), json!(args.timeout_seconds)),
                (
                    "execution".to_string(),
                    Value::String("mcp:peregrine".to_string()),
                ),
            ]),
        ),
        Err(error) => CliStep::failed(
            format!("verify:{}", target.module_name),
            started_at,
            CliDiagnostic::error("mcp:peregrine", error),
        ),
    }
}
