use peregrine_analysis::{
    AnalysisReport as EngineAnalysisReport, AnalysisRequest, DiagnosticSeverity, FindingSeverity,
    GraphKind,
};
use peregrine_analysis_engine::{AnalysisEngine, AnalysisPluginRegistry, RegistryError};
use peregrine_sui_adapter::{SuiAdapterSettings, SuiChainAdapter};
use peregrine_sui_dynamic_analysis::{MovyDynamicAnalyzer, SuiProverDynamicAnalyzer};
use peregrine_sui_move_graph::{MoveProjectGraphs, MoveStateAccessGraph, SuiMoveGraphBuilder};
use peregrine_sui_scanner::SuiScanner;
use peregrine_sui_static_analysis::{
    AnalysisDiagnostic, AnalysisReport, Finding, Metric, RuleMetric, Severity, Span,
    SuiStaticAnalyzer,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

pub fn sui_analysis_engine() -> Result<AnalysisEngine, RegistryError> {
    sui_analysis_engine_with_settings(SuiAdapterSettings::default())
}

pub fn sui_analysis_engine_with_settings(
    settings: SuiAdapterSettings,
) -> Result<AnalysisEngine, RegistryError> {
    let mut registry = AnalysisPluginRegistry::default();
    registry.register_adapter(Arc::new(SuiChainAdapter::new(settings)))?;
    registry.register_scanner(Arc::new(SuiScanner));
    registry.register_graph_builder(Arc::new(SuiMoveGraphBuilder));
    registry.register_static_analyzer(Arc::new(SuiStaticAnalyzer));
    registry.register_dynamic_analyzer(Arc::new(MovyDynamicAnalyzer));
    registry.register_dynamic_analyzer(Arc::new(SuiProverDynamicAnalyzer));
    Ok(AnalysisEngine::new(registry))
}

pub fn run_sui_analysis_blocking(
    request: AnalysisRequest,
    settings: SuiAdapterSettings,
) -> Result<EngineAnalysisReport, String> {
    let engine = sui_analysis_engine_with_settings(settings).map_err(|error| error.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not create Sui analysis runtime: {error}"))?;
    Ok(runtime.block_on(engine.run(request)))
}

pub fn legacy_static_report(report: &EngineAnalysisReport) -> AnalysisReport {
    let findings = report
        .findings
        .iter()
        .map(|finding| Finding {
            rule_id: finding.rule_id.clone(),
            ruleset_id: finding
                .ruleset_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            severity: match finding.severity {
                FindingSeverity::Info => Severity::Info,
                FindingSeverity::Warning => Severity::Warning,
                FindingSeverity::Error => Severity::Error,
            },
            message: finding.message.clone(),
            file: finding
                .span
                .as_ref()
                .map(|span| span.file_path.clone())
                .unwrap_or_default(),
            span: finding.span.as_ref().map(|span| Span {
                start_line: span.start_line,
                end_line: span.end_line,
            }),
            metric: finding.evidence.iter().find_map(|evidence| {
                evidence
                    .metadata
                    .get("metric")
                    .cloned()
                    .and_then(|metric| serde_json::from_value(metric).ok())
            }),
        })
        .collect::<Vec<_>>();
    let metrics = report
        .metrics
        .iter()
        .filter_map(legacy_rule_metric)
        .collect::<Vec<_>>();
    let mut loaded_rulesets = findings
        .iter()
        .map(|finding| finding.ruleset_id.clone())
        .collect::<Vec<_>>();
    loaded_rulesets.sort();
    loaded_rulesets.dedup();

    AnalysisReport {
        findings,
        metrics,
        loaded_rulesets,
        loaded_plugins: report.selected_plugins.clone(),
        diagnostics: report
            .diagnostics
            .iter()
            .map(|diagnostic| AnalysisDiagnostic {
                level: match diagnostic.severity {
                    DiagnosticSeverity::Info => "info",
                    DiagnosticSeverity::Warning => "warning",
                    DiagnosticSeverity::Error => "error",
                }
                .to_string(),
                source: diagnostic
                    .plugin_id
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", diagnostic.stage).to_ascii_lowercase()),
                message: diagnostic.message.clone(),
            })
            .collect(),
    }
}

pub fn legacy_move_project_graphs(
    report: &EngineAnalysisReport,
) -> Result<MoveProjectGraphs, String> {
    Ok(MoveProjectGraphs {
        call_graph: legacy_graph(report, GraphKind::CALL)?,
        type_graph: legacy_graph(report, GraphKind::TYPE)?,
        state_access_graph: legacy_graph(report, GraphKind::STATE_ACCESS)?,
    })
}

pub fn legacy_state_access_graph(
    report: &EngineAnalysisReport,
) -> Result<MoveStateAccessGraph, String> {
    legacy_graph(report, GraphKind::STATE_ACCESS)
}

fn legacy_rule_metric(metric: &peregrine_analysis::AnalysisMetric) -> Option<RuleMetric> {
    let mut name = metric.name.splitn(3, '.');
    let ruleset_id = name.next()?.to_string();
    let rule_id = name.next()?.to_string();
    let metric_name = name.next()?.to_string();
    let value = u32::try_from(metric.value.as_u64()?).ok()?;
    Some(RuleMetric {
        ruleset_id,
        rule_id,
        target: metric
            .metadata
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        file: metric
            .metadata
            .get("file")
            .and_then(Value::as_str)
            .map(str::to_string),
        span: metric
            .metadata
            .get("span")
            .cloned()
            .and_then(|span| serde_json::from_value(span).ok()),
        metric: Metric {
            name: metric_name,
            value,
            threshold: metric
                .metadata
                .get("threshold")
                .and_then(Value::as_u64)
                .and_then(|threshold| u32::try_from(threshold).ok()),
        },
    })
}

fn legacy_graph<T: DeserializeOwned>(
    report: &EngineAnalysisReport,
    kind: &str,
) -> Result<T, String> {
    let graph = report
        .graphs
        .iter()
        .find(|graph| graph.kind.0 == kind)
        .ok_or_else(|| format!("analysis did not produce the `{kind}` graph"))?;
    let value = graph
        .metadata
        .get("legacyGraph")
        .cloned()
        .ok_or_else(|| format!("`{kind}` graph did not include its compatibility payload"))?;
    serde_json::from_value(value)
        .map_err(|error| format!("could not decode `{kind}` graph payload: {error}"))
}
