use crate::{
    output::{
        elapsed_ms, CliDiagnostic, CliDiagnosticSeverity, CliSpan, CliStatus, CliStep,
        EXIT_SUCCESS, EXIT_WORKFLOW_FAILED,
    },
    sui::{args::AnalyzeArgs, project::CliContext},
};
use peregrine_static_analysis::{
    AnalysisConfig, AnalysisDiagnostic, Analyzer, Finding, RuleMetric, Severity,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Instant};

pub fn run_analyze(context: &CliContext, args: &AnalyzeArgs) -> CliStep {
    let started_at = Instant::now();
    let config = match AnalysisConfig::load_from_package(&context.package_root) {
        Ok(config) => config,
        Err(error) => {
            return CliStep::failed(
                "analyze",
                started_at,
                CliDiagnostic::error("analysis-config", error),
            );
        }
    };
    let report = Analyzer::new().analyze_package(&context.package_root, config);
    let mut diagnostics = report
        .diagnostics
        .iter()
        .map(map_analysis_diagnostic)
        .collect::<Vec<_>>();
    diagnostics.extend(report.findings.iter().map(map_finding));

    let has_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == CliDiagnosticSeverity::Error);
    let has_finding = !report.findings.is_empty();
    let should_fail = has_error || (args.fail_on_findings && has_finding);

    CliStep {
        name: "analyze".to_string(),
        status: if should_fail {
            CliStatus::Failed
        } else {
            CliStatus::Passed
        },
        duration_ms: elapsed_ms(started_at),
        exit_code: if should_fail {
            EXIT_WORKFLOW_FAILED
        } else {
            EXIT_SUCCESS
        },
        command: Some("peregrine analyze".to_string()),
        diagnostics,
        metadata: BTreeMap::from([
            ("findingCount".to_string(), json!(report.findings.len())),
            ("metricCount".to_string(), json!(report.metrics.len())),
            ("loadedRulesets".to_string(), json!(report.loaded_rulesets)),
            ("loadedPlugins".to_string(), json!(report.loaded_plugins)),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        details: json!({
            "metrics": report.metrics.iter().map(map_metric).collect::<Vec<_>>(),
        }),
    }
}

fn map_analysis_diagnostic(diagnostic: &AnalysisDiagnostic) -> CliDiagnostic {
    CliDiagnostic {
        severity: match diagnostic.level.as_str() {
            "error" => CliDiagnosticSeverity::Error,
            "warning" => CliDiagnosticSeverity::Warning,
            _ => CliDiagnosticSeverity::Info,
        },
        source: diagnostic.source.clone(),
        code: None,
        message: diagnostic.message.clone(),
        file: None,
        span: None,
    }
}

fn map_finding(finding: &Finding) -> CliDiagnostic {
    CliDiagnostic {
        severity: match finding.severity {
            Severity::Error => CliDiagnosticSeverity::Error,
            Severity::Warning => CliDiagnosticSeverity::Warning,
            Severity::Info => CliDiagnosticSeverity::Info,
        },
        source: finding.ruleset_id.clone(),
        code: Some(finding.rule_id.clone()),
        message: finding.message.clone(),
        file: Some(finding.file.clone()),
        span: finding.span.map(|span| CliSpan {
            start_line: span.start_line,
            end_line: span.end_line,
        }),
    }
}

fn map_metric(metric: &RuleMetric) -> Value {
    json!({
        "rulesetId": metric.ruleset_id,
        "ruleId": metric.rule_id,
        "target": metric.target,
        "file": metric.file,
        "span": metric.span.map(|span| json!({
            "startLine": span.start_line,
            "endLine": span.end_line,
        })),
        "metric": metric.metric,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sui::project::resolve_context;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn analyze_reports_infinite_loop_and_unchecked_return_findings() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("Move.toml"),
            r#"
[package]
name = "demo"
"#,
        )
        .expect("manifest");
        fs::write(
            temp.path().join("sources/m.move"),
            r#"
module demo::m;

fun value(): u64 { 1 }

public fun caller() {
    value();
    loop {
        let x = 1;
    }
}
"#,
        )
        .expect("source");
        let context = resolve_context(temp.path(), ".").expect("context");

        let step = run_analyze(
            &context,
            &AnalyzeArgs {
                fail_on_findings: false,
            },
        );

        let codes = step
            .diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"InfiniteLoop"));
        assert!(codes.contains(&"UncheckedReturn"));
        assert_eq!(step.status, CliStatus::Passed);
    }
}
