use crate::{
    output::{CliDiagnostic, CliStatus, CliStep, EXIT_WORKFLOW_FAILED, elapsed_ms},
    session::McpToolClient,
    sui::{args::NewPackageArgs, runners::process::mcp_command_step},
};
use peregrine_mcp_protocol::{CreatePackageArgs, tool_name};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, time::Instant};

pub fn run_new_package(workspace_root: &Path, args: &NewPackageArgs) -> CliStep {
    let started_at = Instant::now();
    let package_name = match peregrine_mcp_protocol::validate_package_name(&args.package_name) {
        Ok(package_name) => package_name,
        Err(error) => {
            return CliStep::failed(
                "new-package",
                started_at,
                CliDiagnostic::error("sui-adapter", error),
            );
        }
    };
    let target_root = workspace_root.join(package_name);

    if target_root.exists() {
        return CliStep::failed(
            "new-package",
            started_at,
            CliDiagnostic::error(
                "new-package",
                format!("{} already exists.", target_root.display()),
            ),
        );
    }

    let result = McpToolClient::call_blocking::<_, peregrine_mcp_protocol::CommandResult>(
        workspace_root,
        tool_name::CREATE_PACKAGE,
        &CreatePackageArgs {
            project_root: None,
            package_name: package_name.to_string(),
            timeout_ms: None,
        },
    );

    match result {
        Ok(result) => {
            let mut step = mcp_command_step(
                "new-package",
                started_at,
                result,
                BTreeMap::from([
                    (
                        "execution".to_string(),
                        Value::String("mcp:peregrine".to_string()),
                    ),
                    (
                        "packageName".to_string(),
                        Value::String(package_name.to_string()),
                    ),
                    (
                        "packageRoot".to_string(),
                        Value::String(target_root.display().to_string()),
                    ),
                ]),
            );

            if step.status == CliStatus::Passed && !target_root.join("Move.toml").is_file() {
                step.status = CliStatus::Failed;
                step.exit_code = EXIT_WORKFLOW_FAILED;
                step.duration_ms = elapsed_ms(started_at);
                step.diagnostics.push(CliDiagnostic::error(
                    "new-package",
                    format!(
                        "`sui move new` completed but {} does not contain Move.toml.",
                        target_root.display()
                    ),
                ));
            }

            step
        }
        Err(error) => CliStep::failed(
            "new-package",
            started_at,
            CliDiagnostic::error("mcp:peregrine", error),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn new_package_rejects_invalid_project_name_before_running_helper() {
        let temp = tempdir().expect("tempdir");

        let step = run_new_package(
            temp.path(),
            &NewPackageArgs {
                package_name: "../vault".to_string(),
            },
        );

        assert_eq!(step.status, CliStatus::Failed);
        assert_eq!(step.diagnostics[0].source, "sui-adapter");
    }
}
