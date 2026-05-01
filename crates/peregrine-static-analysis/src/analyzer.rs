use std::path::Path;

use peregrine_analysis_core::{
    AnalysisConfig, AnalysisDiagnostic, AnalysisReport, RuleSetProvider,
};
use peregrine_complexity_rules::ComplexityRuleSetProvider;

use crate::{parser::parse_package, plugins::WasmPluginHost};

pub struct Analyzer {
    providers: Vec<Box<dyn RuleSetProvider>>,
    plugin_host: WasmPluginHost,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    pub fn new() -> Self {
        Self {
            providers: vec![Box::new(ComplexityRuleSetProvider)],
            plugin_host: WasmPluginHost::default(),
        }
    }

    pub fn with_provider(mut self, provider: Box<dyn RuleSetProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn analyze_package(
        &self,
        package_path: impl AsRef<Path>,
        config: AnalysisConfig,
    ) -> AnalysisReport {
        let package_path = package_path.as_ref();
        let config = config.with_defaults();
        let mut report = AnalysisReport::default();
        let context = match parse_package(package_path, config.clone()) {
            Ok(context) => context,
            Err(message) => {
                report.diagnostics.push(AnalysisDiagnostic {
                    level: "error".to_string(),
                    source: "parser".to_string(),
                    message,
                });
                return report;
            }
        };

        for provider in &self.providers {
            for ruleset in provider.rule_sets() {
                let ruleset_id = ruleset.id();
                let Some(ruleset_config) = context.config.analysis.rulesets.get(ruleset_id) else {
                    continue;
                };

                if !ruleset_config.is_active() {
                    continue;
                }

                report.loaded_rulesets.push(ruleset_id.to_string());

                for rule in ruleset.rules() {
                    let rule_config = ruleset_config.rule_config(rule.id());

                    if !rule_config.is_active() {
                        continue;
                    }

                    let outcome = rule.analyze(&context, &rule_config);
                    report.findings.extend(outcome.findings);
                    report.metrics.extend(outcome.metrics);
                }
            }
        }

        self.plugin_host.analyze_plugins(&context, &mut report);

        report.loaded_rulesets.sort();
        report.loaded_rulesets.dedup();
        report.loaded_plugins.sort();
        report.loaded_plugins.dedup();
        report
    }
}
