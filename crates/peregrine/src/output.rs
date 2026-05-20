use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Instant};

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

pub fn write_report(report: &CliReport, json_output: bool) -> Result<(), String> {
    let rendered = if json_output {
        serde_json::to_string_pretty(report)
            .map_err(|error| format!("Could not serialize CLI report: {error}"))?
    } else {
        human_report(report)
    };

    println!("{rendered}");
    Ok(())
}

pub fn human_report(report: &CliReport) -> String {
    if let Some(rendered) = direct_human_stdout(report) {
        return rendered;
    }

    let mut lines = Vec::new();

    lines.push(format!(
        "peregrine {}: {} ({} ms)",
        report.command,
        status_label(report.status),
        report.duration_ms
    ));

    if !report.project_root.is_empty() {
        lines.push(format!("Project: {}", report.project_root));
    }

    if !report.package_path.is_empty() {
        lines.push(format!("Package: {}", report.package_path));
    }

    if !report.diagnostics.is_empty() {
        lines.push("Diagnostics:".to_string());
        for diagnostic in &report.diagnostics {
            lines.push(format!("  {}", human_diagnostic(diagnostic)));
        }
    }

    if !report.steps.is_empty() {
        lines.push("Steps:".to_string());
        for step in &report.steps {
            lines.push(format!(
                "  {} {} ({} ms, exit {})",
                status_label(step.status),
                step.name,
                step.duration_ms,
                step.exit_code
            ));

            if let Some(command) = &step.command {
                lines.push(format!("    Command: {command}"));
            }

            if !step.metadata.is_empty() {
                lines.push(format!("    {}", human_metadata(&step.metadata)));
            }

            for diagnostic in &step.diagnostics {
                lines.push(format!("    {}", human_diagnostic(diagnostic)));
            }

            if !step.stdout.trim().is_empty() {
                lines.push("    stdout:".to_string());
                append_indented_block(&mut lines, &step.stdout, 6);
            }

            if !step.stderr.trim().is_empty() {
                lines.push("    stderr:".to_string());
                append_indented_block(&mut lines, &step.stderr, 6);
            }
        }
    }

    lines.join("\n")
}

fn direct_human_stdout(report: &CliReport) -> Option<String> {
    let [step] = report.steps.as_slice() else {
        return None;
    };

    if matches!(
        report.command.as_str(),
        "signatures" | "call-graph" | "object-graph" | "cfg"
    ) && report.status == CliStatus::Passed
        && !step.stdout.trim().is_empty()
    {
        return Some(step.stdout.trim_end().to_string());
    }

    None
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

fn status_label(status: CliStatus) -> &'static str {
    match status {
        CliStatus::Passed => "PASS",
        CliStatus::Failed => "FAIL",
        CliStatus::Skipped => "SKIP",
    }
}

fn severity_label(severity: CliDiagnosticSeverity) -> &'static str {
    match severity {
        CliDiagnosticSeverity::Info => "info",
        CliDiagnosticSeverity::Warning => "warning",
        CliDiagnosticSeverity::Error => "error",
    }
}

fn human_diagnostic(diagnostic: &CliDiagnostic) -> String {
    let mut label = format!(
        "{} [{}]",
        severity_label(diagnostic.severity),
        diagnostic.source
    );

    if let Some(code) = &diagnostic.code {
        label.push_str(&format!(" {code}"));
    }

    if let Some(file) = &diagnostic.file {
        label.push_str(&format!(" {file}"));
    }

    if let Some(span) = &diagnostic.span {
        label.push_str(&format!(":{}-{}", span.start_line, span.end_line));
    }

    format!("{label}: {}", diagnostic.message)
}

fn human_metadata(metadata: &BTreeMap<String, Value>) -> String {
    metadata
        .iter()
        .map(|(key, value)| format!("{key}={}", human_value(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn human_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => {
            let values = values.iter().map(human_value).collect::<Vec<_>>();
            format!("[{}]", values.join(", "))
        }
        Value::Object(_) => "<object>".to_string(),
    }
}

fn append_indented_block(lines: &mut Vec<String>, block: &str, spaces: usize) {
    let indent = " ".repeat(spaces);

    for line in block.trim_end().lines() {
        lines.push(format!("{indent}{line}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_uses_exit_code_and_no_schema_version() {
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

        assert_eq!(report.status, CliStatus::Failed);
        assert_eq!(report.exit_code, EXIT_WORKFLOW_FAILED);
        assert_eq!(report.diagnostics.len(), 1);

        let serialized = serde_json::to_value(&report).expect("json report");
        assert!(serialized.get("schemaVersion").is_none());
    }

    #[test]
    fn human_report_is_default_readable_output() {
        let started_at = Instant::now();
        let report = CliReport::from_steps(
            "analyze",
            started_at,
            "/workspace",
            ".",
            vec![CliStep::skipped("fuzz", "disabled")],
        );

        let rendered = human_report(&report);

        assert!(rendered.contains("peregrine analyze: SKIP"));
        assert!(rendered.contains("Project: /workspace"));
        assert!(rendered.contains("SKIP fuzz"));
    }

    #[test]
    fn signatures_human_report_prints_only_signature_tree() {
        let started_at = Instant::now();
        let report = CliReport::from_steps(
            "signatures",
            started_at,
            "/workspace",
            ".",
            vec![CliStep {
                name: "signatures".to_string(),
                status: CliStatus::Passed,
                duration_ms: 1,
                exit_code: EXIT_SUCCESS,
                command: None,
                diagnostics: Vec::new(),
                metadata: BTreeMap::new(),
                stdout: "package Demo\n|-- module main\n".to_string(),
                stderr: String::new(),
                details: Value::Null,
            }],
        );

        let rendered = human_report(&report);

        assert_eq!(rendered, "package Demo\n|-- module main");
    }
}
