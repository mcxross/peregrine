use peregrine_analysis::{
    AnalysisReport as EngineAnalysisReport, AnalysisRequest, AnalysisStage, DiagnosticSeverity,
    GraphKind,
};
use peregrine_sui_mcp_protocol::{AnalysisReport as ProtocolAnalysisReport, AnalyzeArgs};
use rmcp::ErrorData;
use serde_json::{Value, json};

pub(crate) fn scanner_report_value(report: &EngineAnalysisReport) -> Result<Value, ErrorData> {
    report
        .artifacts
        .as_ref()
        .and_then(|artifacts| artifacts.metadata.get("scannerReports"))
        .and_then(Value::as_array)
        .and_then(|reports| reports.first())
        .cloned()
        .ok_or_else(|| {
            let message = report
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            tool_error(if message.trim().is_empty() {
                "scanner did not produce a report".to_string()
            } else {
                message
            })
        })
}

pub(crate) fn legacy_static_report(
    report: &EngineAnalysisReport,
) -> Result<ProtocolAnalysisReport, ErrorData> {
    let findings = report
        .findings
        .iter()
        .map(|finding| {
            json!({
                "ruleId": finding.rule_id,
                "rulesetId": finding.ruleset_id.as_deref().unwrap_or("unknown"),
                "severity": finding.severity,
                "message": finding.message,
                "file": finding.span.as_ref().map(|span| span.file_path.as_str()).unwrap_or(""),
                "span": finding.span.as_ref().map(|span| json!({
                    "startLine": span.start_line,
                    "endLine": span.end_line,
                })),
                "metric": Value::Null,
            })
        })
        .collect::<Vec<_>>();
    let mut loaded_rulesets = report
        .findings
        .iter()
        .filter_map(|finding| finding.ruleset_id.clone())
        .collect::<Vec<_>>();
    loaded_rulesets.sort();
    loaded_rulesets.dedup();
    let diagnostics = report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "level": match diagnostic.severity {
                    DiagnosticSeverity::Error => "error",
                    DiagnosticSeverity::Warning => "warning",
                    DiagnosticSeverity::Info => "info",
                },
                "source": diagnostic.plugin_id.clone().unwrap_or_else(|| {
                    format!("{:?}", diagnostic.stage).to_ascii_lowercase()
                }),
                "message": diagnostic.message,
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "findings": findings,
        "metrics": [],
        "loadedRulesets": loaded_rulesets,
        "loadedPlugins": report.selected_plugins,
        "diagnostics": diagnostics,
        "graphKinds": report.graphs.iter().map(|graph| graph.kind.clone()).collect::<Vec<GraphKind>>(),
    }))
    .map_err(serialization_error)
}

pub(crate) fn ensure_required_stages(stages: &mut Vec<AnalysisStage>) {
    if stages
        .iter()
        .any(|stage| !matches!(stage, AnalysisStage::Scan))
        && !stages.contains(&AnalysisStage::Scan)
    {
        stages.insert(0, AnalysisStage::Scan);
    }
}

pub(crate) fn apply_analyze_args(request: &mut AnalysisRequest, args: AnalyzeArgs) {
    if !args.stages.is_empty() {
        request.stages = args.stages;
    }
    if !args.graph_kinds.is_empty() {
        request.graph_kinds = args.graph_kinds;
    }
    request.plugin_ids = args.plugin_ids;
    request.dynamic_capabilities = args.dynamic_capabilities;
    if !request.dynamic_capabilities.is_empty() && !request.stages.contains(&AnalysisStage::Dynamic)
    {
        request.stages.push(AnalysisStage::Dynamic);
    }
    ensure_required_stages(&mut request.stages);
    if let Some(limits) = args.limits {
        request.limits = limits;
    }
    request.options = args.options;
}

fn tool_error(message: String) -> ErrorData {
    ErrorData::invalid_params(message, None)
}

fn serialization_error(error: serde_json::Error) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}
