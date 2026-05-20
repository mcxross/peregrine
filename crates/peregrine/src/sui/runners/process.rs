use crate::{
    helper_args::resolve_helper_executable,
    output::{
        command_details, elapsed_ms, CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep,
        EXIT_WORKFLOW_FAILED,
    },
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

#[derive(Clone, Debug)]
pub(super) struct ChildOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub(super) fn run_peregrine_child<I>(args: I) -> Result<ChildOutput, String>
where
    I: IntoIterator<Item = OsString>,
{
    run_peregrine_child_in(args, None)
}

pub(super) fn run_peregrine_child_interactive<I>(args: I) -> Result<ChildOutput, String>
where
    I: IntoIterator<Item = OsString>,
{
    let executable = resolve_helper_executable()?;
    let status = Command::new(executable)
        .args(args)
        .status()
        .map_err(|error| format!("Could not run Peregrine helper process: {error}"))?;

    Ok(ChildOutput {
        status: status.code(),
        stdout: String::new(),
        stderr: String::new(),
    })
}

pub(super) fn run_peregrine_child_in<I>(
    args: I,
    current_dir: Option<&Path>,
) -> Result<ChildOutput, String>
where
    I: IntoIterator<Item = OsString>,
{
    let executable = resolve_helper_executable()?;
    let mut command = Command::new(executable);

    command
        .args(args)
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }

    let output = command
        .output()
        .map_err(|error| format!("Could not run Peregrine helper process: {error}"))?;

    Ok(ChildOutput {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(super) fn command_step(
    name: impl Into<String>,
    started_at: Instant,
    command: Option<String>,
    output: ChildOutput,
    metadata: BTreeMap<String, Value>,
) -> CliStep {
    let status = if output.status == Some(0) {
        CliStatus::Passed
    } else {
        CliStatus::Failed
    };
    let diagnostics = if status == CliStatus::Failed {
        vec![CliDiagnostic {
            severity: CliDiagnosticSeverity::Error,
            source: "process".to_string(),
            code: output.status.map(|status| format!("exit-{status}")),
            message: command_failure_message(&output),
            file: None,
            span: None,
        }]
    } else {
        Vec::new()
    };

    CliStep {
        name: name.into(),
        status,
        duration_ms: elapsed_ms(started_at),
        exit_code: output.status.unwrap_or(EXIT_WORKFLOW_FAILED),
        command,
        diagnostics,
        metadata,
        stdout: output.stdout,
        stderr: output.stderr,
        details: command_details(output.status),
    }
}

fn command_failure_message(output: &ChildOutput) -> String {
    output
        .stderr
        .lines()
        .chain(output.stdout.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .next()
        .unwrap_or("Command failed without diagnostic output.")
        .to_string()
}
