use std::{collections::BTreeSet, path::Path};

use crate::model::{AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, RuleMetric};
use peregrine_plugins::{
    PluginManifestInput, PluginRuntime, PLUGIN_MANIFEST_EXPORT, PLUGIN_SCHEMA_VERSION,
};

use super::{
    resolve_plugin_path, PluginActiveRuleConfig, PluginAnalyzeInput, PluginAnalyzeOutput,
    PluginManifest,
};

const ANALYZE_EXPORT: &str = "peregrine_analyze";

#[derive(Default)]
pub struct AnalysisPluginHost;

impl AnalysisPluginHost {
    pub fn analyze_plugins(&self, context: &AnalysisContext, report: &mut AnalysisReport) {
        for plugin_path in &context.config.analysis.plugins.paths {
            let plugin_path = resolve_plugin_path(&context.package_path, plugin_path);
            let source = format!("plugin:{}", plugin_path.display());

            match self.analyze_plugin_path(context, &plugin_path, &[]) {
                Ok(plugin_report) => {
                    report.loaded_plugins.push(plugin_report.plugin_id);
                    report.findings.extend(plugin_report.findings);
                    report.metrics.extend(plugin_report.metrics);
                }
                Err(message) => report.diagnostics.push(AnalysisDiagnostic {
                    level: "error".to_string(),
                    source,
                    message,
                }),
            }
        }
    }

    pub fn discover_manifest(
        &self,
        plugin_path: impl AsRef<Path>,
    ) -> Result<PluginManifest, String> {
        let mut runtime = PluginRuntime::load_from_path(plugin_path.as_ref())?;
        let input = PluginManifestInput {
            schema_version: PLUGIN_SCHEMA_VERSION,
        };
        let output = runtime.call_json(PLUGIN_MANIFEST_EXPORT, &input)?;
        parse_manifest(&output)
    }

    pub fn analyze_plugin_path(
        &self,
        context: &AnalysisContext,
        plugin_path: &Path,
        active_rules: &[PluginActiveRuleConfig],
    ) -> Result<PluginAnalysisReport, String> {
        let mut runtime = PluginRuntime::load_from_path(plugin_path)?;
        let manifest_input = PluginManifestInput {
            schema_version: PLUGIN_SCHEMA_VERSION,
        };
        let manifest_output = runtime.call_json(PLUGIN_MANIFEST_EXPORT, &manifest_input)?;
        let manifest = parse_manifest(&manifest_output)?;

        let analyze_input = PluginAnalyzeInput {
            schema_version: PLUGIN_SCHEMA_VERSION,
            context: context.clone(),
            config: serde_json::to_value(&context.config)
                .map_err(|error| format!("Could not serialize plugin config: {error}"))?,
            active_rules: active_rules.to_vec(),
        };
        let analyze_output = runtime.call_json(ANALYZE_EXPORT, &analyze_input)?;
        let output: PluginAnalyzeOutput = serde_json::from_str(&analyze_output)
            .map_err(|error| format!("Plugin analysis output is not valid JSON: {error}"))?;

        let active_rule_keys = active_rules
            .iter()
            .map(|rule| (rule.ruleset_id.as_str(), rule.rule_id.as_str()))
            .collect::<BTreeSet<_>>();
        let findings = if active_rule_keys.is_empty() {
            output.findings
        } else {
            output
                .findings
                .into_iter()
                .filter(|finding| {
                    active_rule_keys
                        .contains(&(finding.ruleset_id.as_str(), finding.rule_id.as_str()))
                })
                .collect()
        };
        let metrics = if active_rule_keys.is_empty() {
            output.metrics
        } else {
            output
                .metrics
                .into_iter()
                .filter(|metric| {
                    active_rule_keys
                        .contains(&(metric.ruleset_id.as_str(), metric.rule_id.as_str()))
                })
                .collect()
        };

        Ok(PluginAnalysisReport {
            plugin_id: manifest.plugin_id,
            findings,
            metrics,
        })
    }
}

pub struct PluginAnalysisReport {
    pub plugin_id: String,
    pub findings: Vec<Finding>,
    pub metrics: Vec<RuleMetric>,
}

fn parse_manifest(output: &str) -> Result<PluginManifest, String> {
    let manifest: PluginManifest = serde_json::from_str(output)
        .map_err(|error| format!("Plugin manifest is not valid JSON: {error}"))?;

    if manifest.schema_version != PLUGIN_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported plugin schema version {}. Expected {}.",
            manifest.schema_version, PLUGIN_SCHEMA_VERSION
        ));
    }

    Ok(manifest)
}
