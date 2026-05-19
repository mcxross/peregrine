use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Instant};

pub const CLI_SCHEMA_VERSION: &str = "peregrine.cli.v1";
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_WORKFLOW_FAILED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CliStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CliDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliSpan {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliDiagnostic {
    pub severity: CliDiagnosticSeverity,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<CliSpan>,
}

impl CliDiagnostic {
    pub fn error(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: CliDiagnosticSeverity::Error,
            source: source.into(),
            code: None,
            message: message.into(),
            file: None,
            span: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStep {
    pub name: String,
    pub status: CliStatus,
    pub duration_ms: u64,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<CliDiagnostic>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

impl CliStep {
    pub fn failed(name: impl Into<String>, started_at: Instant, diagnostic: CliDiagnostic) -> Self {
        Self {
            name: name.into(),
            status: CliStatus::Failed,
            duration_ms: elapsed_ms(started_at),
            exit_code: EXIT_WORKFLOW_FAILED,
            command: None,
            diagnostics: vec![diagnostic],
            metadata: BTreeMap::new(),
            stdout: String::new(),
            stderr: String::new(),
            details: Value::Null,
        }
    }

    pub fn skipped(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CliStatus::Skipped,
            duration_ms: 0,
            exit_code: EXIT_SUCCESS,
            command: None,
            diagnostics: vec![CliDiagnostic {
                severity: CliDiagnosticSeverity::Info,
                source: "workflow".to_string(),
                code: Some("Skipped".to_string()),
                message: reason.into(),
                file: None,
                span: None,
            }],
            metadata: BTreeMap::new(),
            stdout: String::new(),
            stderr: String::new(),
            details: Value::Null,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliReport {
    pub schema_version: String,
    pub command: String,
    pub status: CliStatus,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub project_root: String,
    pub package_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<CliDiagnostic>,
    pub steps: Vec<CliStep>,
}

impl CliReport {
    pub fn from_steps(
        command: impl Into<String>,
        started_at: Instant,
        project_root: impl Into<String>,
        package_path: impl Into<String>,
        steps: Vec<CliStep>,
    ) -> Self {
        let status = aggregate_status(&steps);
        let exit_code = status_exit_code(status);
        let diagnostics = steps
            .iter()
            .flat_map(|step| step.diagnostics.iter().cloned())
            .collect();

        Self {
            schema_version: CLI_SCHEMA_VERSION.to_string(),
            command: command.into(),
            status,
            exit_code,
            duration_ms: elapsed_ms(started_at),
            project_root: project_root.into(),
            package_path: package_path.into(),
            diagnostics,
            steps,
        }
    }

    pub fn usage_error(
        command: impl Into<String>,
        started_at: Instant,
        diagnostic: CliDiagnostic,
    ) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION.to_string(),
            command: command.into(),
            status: CliStatus::Failed,
            exit_code: EXIT_USAGE,
            duration_ms: elapsed_ms(started_at),
            project_root: String::new(),
            package_path: String::new(),
            diagnostics: vec![diagnostic],
            steps: Vec::new(),
        }
    }
}

pub fn write_report(report: &CliReport, pretty: bool) -> Result<(), String> {
    let serialized = if pretty {
        serde_json::to_string_pretty(report)
    } else {
        serde_json::to_string(report)
    }
    .map_err(|error| format!("Could not serialize CLI report: {error}"))?;

    println!("{serialized}");
    Ok(())
}

pub fn command_details(status: Option<i32>) -> Value {
    json!({ "processStatus": status })
}

pub fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn aggregate_status(steps: &[CliStep]) -> CliStatus {
    if steps
        .iter()
        .any(|step| matches!(step.status, CliStatus::Failed))
    {
        return CliStatus::Failed;
    }

    if steps
        .iter()
        .all(|step| matches!(step.status, CliStatus::Skipped))
    {
        return CliStatus::Skipped;
    }

    CliStatus::Passed
}

fn status_exit_code(status: CliStatus) -> i32 {
    match status {
        CliStatus::Passed | CliStatus::Skipped => EXIT_SUCCESS,
        CliStatus::Failed => EXIT_WORKFLOW_FAILED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_uses_standard_schema_and_exit_code() {
        let started_at = Instant::now();
        let report = CliReport::from_steps(
            "analyze",
            started_at,
            "/workspace",
            ".",
            vec![CliStep::failed(
                "analyze",
                started_at,
                CliDiagnostic::error("analysis", "failed"),
            )],
        );

        assert_eq!(report.schema_version, CLI_SCHEMA_VERSION);
        assert_eq!(report.status, CliStatus::Failed);
        assert_eq!(report.exit_code, EXIT_WORKFLOW_FAILED);
        assert_eq!(report.diagnostics.len(), 1);
    }
}
