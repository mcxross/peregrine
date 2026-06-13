use crate::{
    AnalysisConfig, AnalysisEngine, AnalysisEngineOptions, AnalysisReport as SuiAnalysisReport,
    Severity,
};
use peregrine_analysis::{
    AnalysisDiagnostic, AnalysisError, AnalysisFuture, AnalysisLimits, AnalysisMetric,
    AnalysisOptions, AnalysisStage, ArtifactBundle, ChainId, DiagnosticSeverity, Evidence, Finding,
    FindingSeverity, PluginDescriptor, PluginOrigin, PluginStage, PropertyGraph, SourceSpan,
    StaticAnalysisOutput, StaticAnalyzer,
};
use peregrine_sui_move_insights::report::package_insights_report_for_package;
use peregrine_sui_move_model::discover_move_packages;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const PLUGIN_ID: &str = "peregrine.sui.static-analysis";

#[derive(Default)]
pub struct SuiStaticAnalyzer;

impl StaticAnalyzer for SuiStaticAnalyzer {
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: PLUGIN_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            chain: ChainId::new("sui"),
            stage: PluginStage::StaticAnalyzer,
            capabilities: vec![
                "ruleBased".to_string(),
                "graphBased".to_string(),
                "moveInsights".to_string(),
                "installedPlugins".to_string(),
                "wasmPlugins".to_string(),
                "nativePlugins".to_string(),
            ],
            origin: PluginOrigin::BuiltIn,
            priority: 100,
        }
    }

    fn analyze<'a>(
        &'a self,
        artifacts: &'a ArtifactBundle,
        graphs: &'a [PropertyGraph],
        options: &'a AnalysisOptions,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, StaticAnalysisOutput> {
        Box::pin(async move {
            let package_root = artifacts.package_root.as_ref().ok_or_else(|| {
                AnalysisError::new(
                    "package_not_materialized",
                    "Sui static analysis requires a locally materialized Move package",
                )
            })?;
            let config = AnalysisConfig::load_from_package(package_root)
                .map_err(|message| AnalysisError::new("analysis_config_failed", message))?;
            let report = AnalysisEngine::new().analyze_package_with_options(
                package_root,
                config,
                analysis_options(options),
            );
            let insight_metrics = move_insight_metrics(package_root)?;
            Ok(convert_report(report, graphs, insight_metrics))
        })
    }
}

fn analysis_options(options: &AnalysisOptions) -> AnalysisEngineOptions {
    AnalysisEngineOptions {
        use_global_plugins: !options
            .get("noGlobalPlugins")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        global_plugin_root: options
            .get("globalPluginRoot")
            .and_then(Value::as_str)
            .map(PathBuf::from),
        extra_plugin_paths: string_array(options.get("pluginPaths"))
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        only_rulesets: string_array(options.get("rulesets")),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn convert_report(
    report: SuiAnalysisReport,
    graphs: &[PropertyGraph],
    insight_metrics: Vec<AnalysisMetric>,
) -> StaticAnalysisOutput {
    let findings = report
        .findings
        .into_iter()
        .enumerate()
        .map(|(index, finding)| {
            let span = finding.span.map(|span| SourceSpan {
                file_path: finding.file.clone(),
                start_line: span.start_line,
                end_line: span.end_line,
                start_byte: 0,
                end_byte: 0,
            });
            Finding {
                id: format!(
                    "{PLUGIN_ID}:{}:{}:{}",
                    finding.rule_id,
                    finding.file,
                    finding.span.map(|span| span.start_line).unwrap_or(index)
                ),
                analyzer_id: PLUGIN_ID.to_string(),
                ruleset_id: Some(finding.ruleset_id),
                rule_id: finding.rule_id,
                severity: match finding.severity {
                    Severity::Info => FindingSeverity::Info,
                    Severity::Warning => FindingSeverity::Warning,
                    Severity::Error => FindingSeverity::Error,
                },
                message: finding.message.clone(),
                span: span.clone(),
                evidence: vec![Evidence {
                    source: PLUGIN_ID.to_string(),
                    artifact_id: None,
                    span,
                    message: finding.message,
                    metadata: json!({"metric": finding.metric}),
                }],
                metadata: json!({"file": finding.file}),
            }
        })
        .collect();
    let metrics = report
        .metrics
        .into_iter()
        .map(|metric| AnalysisMetric {
            analyzer_id: PLUGIN_ID.to_string(),
            name: format!(
                "{}.{}.{}",
                metric.ruleset_id, metric.rule_id, metric.metric.name
            ),
            value: json!(metric.metric.value),
            metadata: json!({
                "target": metric.target,
                "file": metric.file,
                "span": metric.span,
                "threshold": metric.metric.threshold,
            }),
        })
        .chain(std::iter::once(AnalysisMetric {
            analyzer_id: PLUGIN_ID.to_string(),
            name: "inputGraphCount".to_string(),
            value: json!(graphs.len()),
            metadata: json!({
                "graphKinds": graphs.iter().map(|graph| &graph.kind.0).collect::<Vec<_>>(),
            }),
        }))
        .chain(insight_metrics)
        .collect();
    let diagnostics = report
        .diagnostics
        .into_iter()
        .map(|diagnostic| AnalysisDiagnostic {
            stage: AnalysisStage::Static,
            plugin_id: Some(PLUGIN_ID.to_string()),
            severity: if diagnostic.level == "error" {
                DiagnosticSeverity::Error
            } else {
                DiagnosticSeverity::Warning
            },
            code: format!(
                "sui_static_{}",
                diagnostic.source.replace([':', '.', '-'], "_")
            ),
            message: diagnostic.message,
        })
        .collect();

    StaticAnalysisOutput {
        findings,
        metrics,
        diagnostics,
        metadata: json!({
            "loadedRulesets": report.loaded_rulesets,
            "loadedPlugins": report.loaded_plugins,
        }),
    }
}

fn move_insight_metrics(package_root: &Path) -> Result<Vec<AnalysisMetric>, AnalysisError> {
    discover_move_packages(package_root, /*recursive*/ true)
        .into_iter()
        .map(|package| {
            let root = if package.path.is_empty() {
                package_root.to_path_buf()
            } else {
                package_root.join(&package.path)
            };
            let report = package_insights_report_for_package(
                &package,
                Some(root.clone()),
                Some(root.join("build")),
            );
            let value = serde_json::to_value(report).map_err(|error| {
                AnalysisError::new(
                    "move_insights_serialization_failed",
                    format!("could not serialize Move insights: {error}"),
                )
            })?;
            Ok(AnalysisMetric {
                analyzer_id: PLUGIN_ID.to_string(),
                name: format!("packageInsights.{}", package.name),
                value,
                metadata: json!({"packagePath": package.path}),
            })
        })
        .collect()
}
