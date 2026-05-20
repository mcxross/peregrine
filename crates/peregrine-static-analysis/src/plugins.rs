use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::model::{AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, RuleMetric};
pub use peregrine_plugins::{resolve_plugin_path, PluginManifestInput};
use peregrine_plugins::{
    InstalledPlugin, PluginInstallManifest, PluginKind, PluginRegistry, PluginRegistryFile,
    PluginRuntimeKind, WasmPluginRuntime, PLUGIN_SCHEMA_VERSION,
};
use peregrine_types::analysis::{
    RuleConfig, RuleConfigProperty, RuleConfigValueKind, RuleMetadata, RuleSetMetadata, Severity,
};

/// Stable v1 WASM ABI for third-party rulesets.
///
/// Plugins export linear `memory`, `peregrine_alloc(len: i32) -> i32`,
/// `peregrine_plugin_manifest(ptr: i32, len: i32) -> i64`, and
/// `peregrine_analyze(ptr: i32, len: i32) -> i64`. The host writes UTF-8 JSON
/// input into plugin memory. The plugin returns a packed `(ptr, len)` pair where
/// the high 32 bits are the output pointer and the low 32 bits are the output
/// length. The output bytes must also be UTF-8 JSON.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAnalyzeInput {
    pub schema_version: u32,
    pub context: AnalysisContext,
    pub config: Value,
    #[serde(default)]
    pub active_rules: Vec<PluginActiveRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub schema_version: u32,
    pub plugin_id: String,
    pub version: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub rulesets: Vec<PluginRuleSetManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuleSetManifest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub rules: Vec<PluginRuleManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuleManifest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default_severity: Option<Severity>,
    #[serde(default)]
    pub config_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginActiveRuleConfig {
    pub ruleset_id: String,
    pub rule_id: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginAnalyzeOutput {
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub metrics: Vec<RuleMetric>,
}

#[derive(Default)]
pub struct WasmPluginHost;

impl WasmPluginHost {
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
        let mut runtime = WasmPluginRuntime::load(plugin_path.as_ref())?;
        let input = PluginManifestInput {
            schema_version: PLUGIN_SCHEMA_VERSION,
        };
        let output = runtime.call_json("peregrine_plugin_manifest", &input)?;
        let manifest: PluginManifest = serde_json::from_str(&output)
            .map_err(|error| format!("Plugin manifest is not valid JSON: {error}"))?;

        if manifest.schema_version != PLUGIN_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported plugin schema version {}. Expected {}.",
                manifest.schema_version, PLUGIN_SCHEMA_VERSION
            ));
        }

        Ok(manifest)
    }

    pub fn analyze_plugin_path(
        &self,
        context: &AnalysisContext,
        plugin_path: &Path,
        active_rules: &[PluginActiveRuleConfig],
    ) -> Result<PluginAnalysisReport, String> {
        let mut runtime = WasmPluginRuntime::load(plugin_path)?;
        let manifest_input = PluginManifestInput {
            schema_version: PLUGIN_SCHEMA_VERSION,
        };
        let manifest_output = runtime.call_json("peregrine_plugin_manifest", &manifest_input)?;
        let manifest: PluginManifest = serde_json::from_str(&manifest_output)
            .map_err(|error| format!("Plugin manifest is not valid JSON: {error}"))?;

        if manifest.schema_version != PLUGIN_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported plugin schema version {}. Expected {}.",
                manifest.schema_version, PLUGIN_SCHEMA_VERSION
            ));
        }

        let analyze_input = PluginAnalyzeInput {
            schema_version: PLUGIN_SCHEMA_VERSION,
            context: context.clone(),
            config: serde_json::to_value(&context.config)
                .map_err(|error| format!("Could not serialize plugin config: {error}"))?,
            active_rules: active_rules.to_vec(),
        };
        let analyze_output = runtime.call_json("peregrine_analyze", &analyze_input)?;
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

pub type InstalledAnalyzerPlugin = InstalledPlugin;
pub type AnalyzerPluginRegistryFile = PluginRegistryFile;

#[derive(Debug, Clone)]
pub struct AnalyzerPluginRegistry {
    registry: PluginRegistry,
}

impl AnalyzerPluginRegistry {
    pub fn default_root() -> Result<PathBuf, String> {
        PluginRegistry::default_root()
    }

    pub fn default() -> Result<Self, String> {
        Ok(Self {
            registry: PluginRegistry::default()?,
        })
    }

    pub fn at_root(root: impl Into<PathBuf>) -> Self {
        Self {
            registry: PluginRegistry::at_root(root),
        }
    }

    pub fn root(&self) -> &Path {
        self.registry.root()
    }

    pub fn registry_path(&self) -> PathBuf {
        self.registry.registry_path()
    }

    pub fn install_dir(&self) -> PathBuf {
        self.registry.install_dir()
    }

    pub fn load(&self) -> Result<AnalyzerPluginRegistryFile, String> {
        self.registry.load()
    }

    pub fn save(&self, registry: &AnalyzerPluginRegistryFile) -> Result<(), String> {
        self.registry.save(registry)
    }

    pub fn list_plugins(&self) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
        self.registry
            .list_plugins_by_kind(&PluginKind::static_analysis())
    }

    pub fn enabled_plugin_paths(&self) -> Result<Vec<PathBuf>, String> {
        self.registry
            .enabled_plugin_paths_for_kind(&PluginKind::static_analysis())
    }

    pub fn install_plugin(
        &self,
        source_path: impl AsRef<Path>,
        host: &WasmPluginHost,
    ) -> Result<InstalledAnalyzerPlugin, String> {
        let source_path = source_path.as_ref();
        let manifest = host.discover_manifest(source_path)?;
        let install_manifest = PluginInstallManifest {
            plugin_id: manifest.plugin_id.clone(),
            version: manifest.version.clone(),
            kind: PluginKind::static_analysis(),
            runtime: PluginRuntimeKind::Wasm,
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            manifest: serde_json::to_value(&manifest).map_err(|error| {
                format!("Could not serialize analyzer plugin manifest: {error}")
            })?,
        };
        self.registry.install_plugin(source_path, install_manifest)
    }

    pub fn remove_plugin(&self, plugin_id: &str) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
        self.registry
            .remove_plugin(&PluginKind::static_analysis(), plugin_id)
    }

    pub fn set_plugin_enabled(
        &self,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
        self.registry
            .set_plugin_enabled(&PluginKind::static_analysis(), plugin_id, enabled)
    }
}

pub fn plugin_manifest_rulesets(manifest: &PluginManifest) -> Vec<RuleSetMetadata> {
    manifest
        .rulesets
        .iter()
        .map(|ruleset| RuleSetMetadata {
            id: ruleset.id.clone(),
            name: ruleset.name.clone().unwrap_or_else(|| ruleset.id.clone()),
            description: ruleset.description.clone().unwrap_or_default(),
            bundled: false,
            plugin_id: Some(manifest.plugin_id.clone()),
            active: true,
            rules: ruleset
                .rules
                .iter()
                .map(|rule| RuleMetadata {
                    id: rule.id.clone(),
                    name: rule.name.clone().unwrap_or_else(|| rule.id.clone()),
                    description: rule.description.clone().unwrap_or_default(),
                    active: true,
                    default_severity: rule.default_severity.clone().unwrap_or(Severity::Warning),
                    configured_severity: None,
                    config_schema: rule
                        .config_keys
                        .iter()
                        .map(|key| RuleConfigProperty {
                            key: key.clone(),
                            value_kind: RuleConfigValueKind::String,
                            description: String::new(),
                            default_value: None,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

pub fn plugin_rule_config_value(config: &RuleConfig) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisConfig, Analyzer, Metric, Severity};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn fixture_plugin_manifest_is_discovered() {
        let package = move_package();
        let plugin_path = write_fixture_plugin(package.path(), "plugin.wasm");
        let manifest = WasmPluginHost
            .discover_manifest(&plugin_path)
            .expect("manifest");

        assert_eq!(manifest.plugin_id, "fixture-plugin");
        assert_eq!(manifest.rulesets[0].id, "fixture");
    }

    #[test]
    fn fixture_plugin_findings_are_merged() {
        let package = move_package();
        let plugin_path = write_fixture_plugin(package.path(), "plugin.wasm");
        let mut config = AnalysisConfig::default();
        config.analysis.plugins.paths =
            vec![plugin_path.strip_prefix(package.path()).unwrap().into()];

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(report
            .loaded_plugins
            .iter()
            .any(|plugin| plugin == "fixture-plugin"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule_id == "FixtureRule"));
    }

    #[test]
    fn plugin_failures_are_reported_as_diagnostics() {
        let package = move_package();
        let mut config = AnalysisConfig::default();
        config.analysis.plugins.paths = vec![PathBuf::from("missing.wasm")];

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source.contains("missing.wasm")));
    }

    #[test]
    fn registry_installs_and_disables_wasm_plugins() {
        let package = move_package();
        let plugin_path = write_fixture_plugin(package.path(), "plugin.wasm");
        let registry_root = tempfile::tempdir().expect("registry");
        let registry = AnalyzerPluginRegistry::at_root(registry_root.path());
        let installed = registry
            .install_plugin(&plugin_path, &WasmPluginHost)
            .expect("install");

        assert_eq!(installed.plugin_id, "fixture-plugin");
        assert!(installed.enabled);
        assert!(installed.path.is_file());
        assert_eq!(registry.enabled_plugin_paths().expect("enabled").len(), 1);

        registry
            .set_plugin_enabled("fixture-plugin", false)
            .expect("disable");

        assert!(registry
            .enabled_plugin_paths()
            .expect("enabled after disable")
            .is_empty());
    }

    fn move_package() -> TempDir {
        let temp = tempfile::tempdir().expect("temp package");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.0.1\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources dir");
        fs::write(
            temp.path().join("sources/m.move"),
            r#"
module demo::m {
    fun id(): u64 { 1 }
}
"#,
        )
        .expect("source");
        temp
    }

    fn write_fixture_plugin(package_path: &Path, name: &str) -> PathBuf {
        let manifest = r#"{"schemaVersion":1,"pluginId":"fixture-plugin","version":"0.1.0","rulesets":[{"id":"fixture","rules":[{"id":"FixtureRule","configKeys":[]}]}]}"#;
        let finding = serde_json::to_string(&Finding {
            rule_id: "FixtureRule".to_string(),
            ruleset_id: "fixture".to_string(),
            severity: Severity::Warning,
            message: "fixture finding".to_string(),
            file: "sources/m.move".to_string(),
            span: None,
            metric: Some(Metric {
                name: "fixture".to_string(),
                value: 1,
                threshold: Some(0),
            }),
        })
        .expect("finding json");
        let analyze = format!(r#"{{"findings":[{finding}],"metrics":[]}}"#);
        let wasm = fixture_wasm(manifest, &analyze);
        let path = package_path.join(name);

        fs::write(&path, wasm).expect("wasm plugin");
        path
    }

    fn fixture_wasm(manifest: &str, analyze: &str) -> Vec<u8> {
        let manifest_offset = 16_u32;
        let analyze_offset = 512_u32;
        let wat = format!(
            r#"
(module
  (memory (export "memory") 1)
  (data (i32.const {manifest_offset}) "{manifest}")
  (data (i32.const {analyze_offset}) "{analyze}")
  (func (export "peregrine_alloc") (param i32) (result i32)
    i32.const 2048)
  (func (export "peregrine_plugin_manifest") (param i32 i32) (result i64)
    i64.const {manifest_result})
  (func (export "peregrine_analyze") (param i32 i32) (result i64)
    i64.const {analyze_result})
)
"#,
            manifest = wat_escape(manifest),
            analyze = wat_escape(analyze),
            manifest_result = pack_result(manifest_offset, manifest.len() as u32),
            analyze_result = pack_result(analyze_offset, analyze.len() as u32),
        );

        wat::parse_str(&wat).expect("fixture wat")
    }

    fn pack_result(ptr: u32, len: u32) -> u64 {
        ((ptr as u64) << 32) | len as u64
    }

    fn wat_escape(source: &str) -> String {
        source
            .bytes()
            .map(|byte| match byte {
                b'"' => "\\22".to_string(),
                b'\\' => "\\5c".to_string(),
                byte if byte.is_ascii_graphic() || byte == b' ' => (byte as char).to_string(),
                byte => format!("\\{byte:02x}"),
            })
            .collect()
    }
}
