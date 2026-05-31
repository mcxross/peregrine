use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::{
    parser::parse_package,
    plugins::{
        AnalysisPluginHost, AnalyzerPluginRegistry, PluginActiveRuleConfig, PluginManifest,
        plugin_manifest_rulesets, plugin_rule_config_value, resolve_plugin_path,
    },
    sui::{SuiRuleSetProvider, rules::complexity::ComplexityRuleSetProvider},
};
use peregrine_types::analysis::{
    AnalysisConfig, AnalysisContext, AnalysisDiagnostic, AnalysisReport, AnalysisRuleCatalog,
    RuleConfig, RuleSetMetadata, RuleSetProvider,
};

pub struct AnalysisEngine {
    providers: Vec<Box<dyn RuleSetProvider>>,
    plugin_host: AnalysisPluginHost,
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisEngine {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(ComplexityRuleSetProvider),
                Box::new(SuiRuleSetProvider),
            ],
            plugin_host: AnalysisPluginHost,
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
        self.analyze_package_with_options(package_path, config, AnalysisEngineOptions::default())
    }

    pub fn analyze_package_with_options(
        &self,
        package_path: impl AsRef<Path>,
        config: AnalysisConfig,
        options: AnalysisEngineOptions,
    ) -> AnalysisReport {
        let package_path = package_path.as_ref();
        let mut config = config.with_defaults();
        if !options.use_global_plugins {
            config.analysis.plugins.use_global = false;
        }

        let mut report = AnalysisReport::default();
        let context = match parse_package(package_path, config) {
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

        self.run_bundled_rules(&context, &options, &mut report);
        self.run_plugin_rules(&context, &options, &mut report);
        finalize_report(report)
    }

    pub fn catalog(
        &self,
        package_path: impl AsRef<Path>,
        config: AnalysisConfig,
    ) -> AnalysisRuleCatalog {
        self.catalog_with_options(package_path, config, AnalysisEngineOptions::default())
    }

    pub fn catalog_with_options(
        &self,
        package_path: impl AsRef<Path>,
        config: AnalysisConfig,
        options: AnalysisEngineOptions,
    ) -> AnalysisRuleCatalog {
        let package_path = package_path.as_ref();
        let mut config = config.with_defaults();
        if !options.use_global_plugins {
            config.analysis.plugins.use_global = false;
        }

        let mut catalog = AnalysisRuleCatalog {
            rulesets: self.bundled_rule_catalog(),
            loaded_plugins: Vec::new(),
            diagnostics: Vec::new(),
        };

        for plugin_path in
            self.plugin_paths(package_path, &config, &options, &mut catalog.diagnostics)
        {
            let source = format!("plugin:{}", plugin_path.display());
            match self.plugin_host.discover_manifest(&plugin_path) {
                Ok(manifest) => {
                    catalog.loaded_plugins.push(manifest.plugin_id.clone());
                    catalog.rulesets.extend(plugin_manifest_rulesets(&manifest));
                }
                Err(message) => catalog.diagnostics.push(AnalysisDiagnostic {
                    level: "error".to_string(),
                    source,
                    message,
                }),
            }
        }

        catalog.rulesets.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then(left.plugin_id.cmp(&right.plugin_id))
        });
        filter_ruleset_catalog(&mut catalog.rulesets, &options.only_rulesets);
        apply_catalog_config(&mut catalog.rulesets, &config);
        catalog.loaded_plugins.sort();
        catalog.loaded_plugins.dedup();
        catalog
    }

    pub fn bundled_rule_catalog(&self) -> Vec<RuleSetMetadata> {
        let mut rulesets = self
            .providers
            .iter()
            .flat_map(|provider| provider.rule_sets())
            .map(|ruleset| {
                let mut metadata = ruleset.metadata();
                metadata.bundled = true;
                metadata.plugin_id = None;
                metadata
            })
            .collect::<Vec<_>>();

        rulesets.sort_by(|left, right| left.id.cmp(&right.id));
        rulesets
    }

    fn run_bundled_rules(
        &self,
        context: &AnalysisContext,
        options: &AnalysisEngineOptions,
        report: &mut AnalysisReport,
    ) {
        for provider in &self.providers {
            for ruleset in provider.rule_sets() {
                let ruleset_id = ruleset.id();

                if !ruleset_filter_allows(&options.only_rulesets, ruleset_id) {
                    continue;
                }

                let ruleset_config = context
                    .config
                    .analysis
                    .rulesets
                    .get(ruleset_id)
                    .cloned()
                    .unwrap_or_default();

                if !ruleset_config.is_active() {
                    continue;
                }

                report.loaded_rulesets.push(ruleset_id.to_string());

                for rule in ruleset.rules() {
                    let rule_config = ruleset_config.rule_config(rule.id());

                    if !rule_config.is_active() {
                        continue;
                    }

                    let mut outcome = rule.analyze(context, &rule_config);
                    apply_severity_override(&mut outcome.findings, &rule_config);
                    report.findings.extend(outcome.findings);
                    report.metrics.extend(outcome.metrics);
                }
            }
        }
    }

    fn run_plugin_rules(
        &self,
        context: &AnalysisContext,
        options: &AnalysisEngineOptions,
        report: &mut AnalysisReport,
    ) {
        let plugin_paths = self.plugin_paths(
            &context.package_path,
            &context.config,
            options,
            &mut report.diagnostics,
        );

        for plugin_path in plugin_paths {
            let source = format!("plugin:{}", plugin_path.display());
            let manifest = match self.plugin_host.discover_manifest(&plugin_path) {
                Ok(manifest) => manifest,
                Err(message) => {
                    report.diagnostics.push(AnalysisDiagnostic {
                        level: "error".to_string(),
                        source,
                        message,
                    });
                    continue;
                }
            };

            report.loaded_plugins.push(manifest.plugin_id.clone());
            let active_rules = active_plugin_rules(context, &manifest, &options.only_rulesets);

            if active_rules.is_empty() {
                continue;
            }

            for active_rule in &active_rules {
                report.loaded_rulesets.push(active_rule.ruleset_id.clone());
            }

            match self
                .plugin_host
                .analyze_plugin_path(context, &plugin_path, &active_rules)
            {
                Ok(mut plugin_report) => {
                    apply_plugin_severity_overrides(
                        &mut plugin_report.findings,
                        context,
                        &active_rules,
                    );
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

    fn plugin_paths(
        &self,
        package_path: &Path,
        config: &AnalysisConfig,
        options: &AnalysisEngineOptions,
        diagnostics: &mut Vec<AnalysisDiagnostic>,
    ) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut seen = BTreeSet::new();

        if config.analysis.plugins.use_global && options.use_global_plugins {
            let registry = options
                .global_plugin_root
                .as_ref()
                .map(|root| Ok(AnalyzerPluginRegistry::at_root(root)))
                .unwrap_or_else(AnalyzerPluginRegistry::default);

            match registry.and_then(|registry| registry.enabled_plugin_paths()) {
                Ok(plugin_paths) => {
                    for plugin_path in plugin_paths {
                        push_plugin_path(&mut paths, &mut seen, plugin_path);
                    }
                }
                Err(message) => diagnostics.push(AnalysisDiagnostic {
                    level: "warning".to_string(),
                    source: "analyzer-plugin-registry".to_string(),
                    message,
                }),
            }
        }

        for plugin_path in &config.analysis.plugins.paths {
            push_plugin_path(
                &mut paths,
                &mut seen,
                resolve_plugin_path(package_path, plugin_path),
            );
        }

        for plugin_path in &options.extra_plugin_paths {
            push_plugin_path(
                &mut paths,
                &mut seen,
                resolve_plugin_path(package_path, plugin_path),
            );
        }

        paths
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisEngineOptions {
    pub use_global_plugins: bool,
    pub global_plugin_root: Option<PathBuf>,
    pub extra_plugin_paths: Vec<PathBuf>,
    pub only_rulesets: Vec<String>,
}

impl Default for AnalysisEngineOptions {
    fn default() -> Self {
        Self {
            use_global_plugins: true,
            global_plugin_root: None,
            extra_plugin_paths: Vec::new(),
            only_rulesets: Vec::new(),
        }
    }
}

impl AnalysisEngineOptions {
    pub fn without_global_plugins() -> Self {
        Self {
            use_global_plugins: false,
            ..Self::default()
        }
    }
}

fn active_plugin_rules(
    context: &AnalysisContext,
    manifest: &PluginManifest,
    only_rulesets: &[String],
) -> Vec<PluginActiveRuleConfig> {
    let mut active_rules = Vec::new();

    for ruleset in &manifest.rulesets {
        if !ruleset_filter_allows(only_rulesets, &ruleset.id) {
            continue;
        }

        let ruleset_config = context
            .config
            .analysis
            .rulesets
            .get(&ruleset.id)
            .cloned()
            .unwrap_or_default();

        if !ruleset_config.is_active() {
            continue;
        }

        for rule in &ruleset.rules {
            let rule_config = ruleset_config.rule_config(&rule.id);

            if !rule_config.is_active() {
                continue;
            }

            active_rules.push(PluginActiveRuleConfig {
                ruleset_id: ruleset.id.clone(),
                rule_id: rule.id.clone(),
                config: plugin_rule_config_value(&rule_config),
            });
        }
    }

    active_rules
}

fn ruleset_filter_allows(only_rulesets: &[String], ruleset_id: &str) -> bool {
    only_rulesets.is_empty()
        || only_rulesets
            .iter()
            .any(|candidate| candidate == ruleset_id)
}

fn filter_ruleset_catalog(rulesets: &mut Vec<RuleSetMetadata>, only_rulesets: &[String]) {
    if only_rulesets.is_empty() {
        return;
    }

    rulesets.retain(|ruleset| ruleset_filter_allows(only_rulesets, &ruleset.id));
}

fn apply_severity_override(
    findings: &mut [peregrine_types::analysis::Finding],
    config: &RuleConfig,
) {
    if let Some(severity) = &config.severity {
        for finding in findings {
            finding.severity = severity.clone();
        }
    }
}

fn apply_plugin_severity_overrides(
    findings: &mut [peregrine_types::analysis::Finding],
    context: &AnalysisContext,
    active_rules: &[PluginActiveRuleConfig],
) {
    let active_rule_configs = active_rules
        .iter()
        .filter_map(|rule| {
            context
                .config
                .analysis
                .rulesets
                .get(&rule.ruleset_id)
                .map(|ruleset_config| {
                    (
                        (rule.ruleset_id.clone(), rule.rule_id.clone()),
                        ruleset_config.rule_config(&rule.rule_id),
                    )
                })
        })
        .collect::<BTreeMap<_, _>>();

    for finding in findings {
        if let Some(rule_config) =
            active_rule_configs.get(&(finding.ruleset_id.clone(), finding.rule_id.clone()))
        {
            apply_severity_override(std::slice::from_mut(finding), rule_config);
        }
    }
}

fn push_plugin_path(paths: &mut Vec<PathBuf>, seen: &mut BTreeSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
}

fn apply_catalog_config(rulesets: &mut [RuleSetMetadata], config: &AnalysisConfig) {
    for ruleset in rulesets {
        let ruleset_config = config.analysis.rulesets.get(&ruleset.id).cloned();
        ruleset.active = ruleset_config
            .as_ref()
            .map(|config| config.is_active())
            .unwrap_or(true);

        for rule in &mut ruleset.rules {
            let rule_config = ruleset_config
                .as_ref()
                .map(|config| config.rule_config(&rule.id))
                .unwrap_or_default();
            rule.active = rule_config.is_active();
            rule.configured_severity = rule_config.severity;
        }
    }
}

fn finalize_report(mut report: AnalysisReport) -> AnalysisReport {
    report.loaded_rulesets.sort();
    report.loaded_rulesets.dedup();
    report.loaded_plugins.sort();
    report.loaded_plugins.dedup();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn engine_loads_bundled_rule_catalog() {
        let package = move_package("module demo::m;");
        let catalog = AnalysisEngine::new().catalog_with_options(
            package.path(),
            AnalysisConfig::default(),
            AnalysisEngineOptions::without_global_plugins(),
        );

        assert!(
            catalog
                .rulesets
                .iter()
                .any(|ruleset| ruleset.id == "complexity")
        );
        assert!(
            catalog
                .rulesets
                .iter()
                .any(|ruleset| ruleset.id == "bool_judgement")
        );
        assert!(
            catalog
                .rulesets
                .iter()
                .any(|ruleset| ruleset.id == "unchecked_return")
        );
    }

    #[test]
    fn inactive_rule_is_skipped() {
        let package = move_package(
            r#"
module demo::m;

public fun flags(flag: bool) {
    if (flag == true) {};
}
"#,
        );
        let config = toml::from_str::<AnalysisConfig>(
            r#"
[analysis.rulesets.bool_judgement]
active = false
"#,
        )
        .expect("config");

        let report = AnalysisEngine::new().analyze_package_with_options(
            package.path(),
            config,
            AnalysisEngineOptions::without_global_plugins(),
        );

        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.rule_id == "bool_judgement")
        );
    }

    #[test]
    fn rule_severity_override_is_applied() {
        let package = move_package(
            r#"
module demo::m;

public fun flags(flag: bool) {
    if (flag == true) {};
}
"#,
        );
        let config = toml::from_str::<AnalysisConfig>(
            r#"
[analysis.rulesets.bool_judgement]
severity = "error"
"#,
        )
        .expect("config");

        let report = AnalysisEngine::new().analyze_package_with_options(
            package.path(),
            config,
            AnalysisEngineOptions::without_global_plugins(),
        );

        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "bool_judgement"
                && finding.severity == peregrine_types::analysis::Severity::Error
        }));
    }

    fn move_package(source: &str) -> tempfile::TempDir {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.0.1\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(temp.path().join("sources/m.move"), source).expect("source");
        temp
    }
}
