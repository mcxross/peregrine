use crate::{
    helper_args::BUNDLED_SUI_HELPER_ARG,
    output::{elapsed_ms, CliDiagnostic, CliStatus, CliStep, EXIT_WORKFLOW_FAILED},
    sui::{
        args::NewPackageArgs,
        runners::process::{command_step, run_peregrine_child_in},
    },
};
use peregrine_adapters::sui::{SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings};
use serde_json::Value;
use std::{collections::BTreeMap, ffi::OsString, path::Path, time::Instant};

pub fn run_new_package(workspace_root: &Path, args: &NewPackageArgs) -> CliStep {
    let started_at = Instant::now();
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = match adapter.move_new_command(&args.package_name) {
        Ok(command) => command,
        Err(error) => {
            return CliStep::failed(
                "new-package",
                started_at,
                CliDiagnostic::error("sui-adapter", error.to_string()),
            );
        }
    };
    let target_root = workspace_root.join(&command.project_name);

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

    let helper_args = std::iter::once(OsString::from(BUNDLED_SUI_HELPER_ARG))
        .chain(command.bundled_args())
        .collect::<Vec<_>>();

    match run_peregrine_child_in(helper_args, Some(workspace_root)) {
        Ok(output) => {
            let mut step = command_step(
                "new-package",
                started_at,
                Some(command.display),
                output,
                BTreeMap::from([
                    (
                        "execution".to_string(),
                        Value::String("bundled-sui".to_string()),
                    ),
                    (
                        "packageName".to_string(),
                        Value::String(command.project_name.clone()),
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
            CliDiagnostic::error("new-package", error),
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
