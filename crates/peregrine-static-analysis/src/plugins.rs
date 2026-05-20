use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use wasmtime::{Engine, Instance, Memory, Module, Store};

use crate::model::{AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, RuleMetric};
use peregrine_types::analysis::{
    RuleConfig, RuleConfigProperty, RuleConfigValueKind, RuleMetadata, RuleSetMetadata, Severity,
};

pub const PLUGIN_SCHEMA_VERSION: u32 = 1;
const REGISTRY_VERSION: u32 = 1;
const APP_CONFIG_DIR_NAME: &str = "xyz.mcxross.peregrine";
const PLUGIN_REGISTRY_FILE: &str = "analyzer-plugins.json";
const PLUGIN_INSTALL_DIR: &str = "analyzer-plugins";

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
pub struct PluginManifestInput {
    pub schema_version: u32,
}

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
        let mut runtime = PluginRuntime::load(plugin_path.as_ref())?;
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
        let mut runtime = PluginRuntime::load(plugin_path)?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAnalyzerPlugin {
    pub plugin_id: String,
    pub version: String,
    pub path: PathBuf,
    pub checksum: String,
    pub enabled: bool,
    pub installed_at_unix_ms: u64,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerPluginRegistryFile {
    pub version: u32,
    pub plugins: Vec<InstalledAnalyzerPlugin>,
}

impl Default for AnalyzerPluginRegistryFile {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            plugins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzerPluginRegistry {
    root: PathBuf,
}

impl AnalyzerPluginRegistry {
    pub fn default_root() -> Result<PathBuf, String> {
        if let Ok(config_dir) = std::env::var("PEREGRINE_CONFIG_DIR") {
            let trimmed = config_dir.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed));
            }
        }

        platform_config_root().map(|root| root.join(APP_CONFIG_DIR_NAME))
    }

    pub fn default() -> Result<Self, String> {
        Ok(Self {
            root: Self::default_root()?,
        })
    }

    pub fn at_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn registry_path(&self) -> PathBuf {
        self.root.join(PLUGIN_REGISTRY_FILE)
    }

    pub fn install_dir(&self) -> PathBuf {
        self.root.join(PLUGIN_INSTALL_DIR)
    }

    pub fn load(&self) -> Result<AnalyzerPluginRegistryFile, String> {
        let path = self.registry_path();

        if !path.is_file() {
            return Ok(AnalyzerPluginRegistryFile::default());
        }

        let contents = fs::read_to_string(&path).map_err(|error| {
            format!(
                "Could not read analyzer plugin registry {}: {error}",
                path.display()
            )
        })?;
        let mut registry =
            serde_json::from_str::<AnalyzerPluginRegistryFile>(&contents).map_err(|error| {
                format!(
                    "Could not parse analyzer plugin registry {}: {error}",
                    path.display()
                )
            })?;

        if registry.version != REGISTRY_VERSION {
            return Err(format!(
                "Unsupported analyzer plugin registry version {}. Expected {}.",
                registry.version, REGISTRY_VERSION
            ));
        }

        registry.plugins.sort_by(|left, right| {
            left.plugin_id
                .cmp(&right.plugin_id)
                .then(left.version.cmp(&right.version))
        });

        Ok(registry)
    }

    pub fn save(&self, registry: &AnalyzerPluginRegistryFile) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|error| {
            format!(
                "Could not create analyzer plugin registry directory {}: {error}",
                self.root.display()
            )
        })?;
        let contents = serde_json::to_string_pretty(registry)
            .map_err(|error| format!("Could not serialize analyzer plugin registry: {error}"))?;
        fs::write(self.registry_path(), contents).map_err(|error| {
            format!(
                "Could not write analyzer plugin registry {}: {error}",
                self.registry_path().display()
            )
        })
    }

    pub fn list_plugins(&self) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
        Ok(self.load()?.plugins)
    }

    pub fn enabled_plugin_paths(&self) -> Result<Vec<PathBuf>, String> {
        Ok(self
            .load()?
            .plugins
            .into_iter()
            .filter(|plugin| plugin.enabled)
            .map(|plugin| plugin.path)
            .collect())
    }

    pub fn install_plugin(
        &self,
        source_path: impl AsRef<Path>,
        host: &WasmPluginHost,
    ) -> Result<InstalledAnalyzerPlugin, String> {
        let source_path = source_path.as_ref();
        let manifest = host.discover_manifest(source_path)?;
        let bytes = fs::read(source_path).map_err(|error| {
            format!(
                "Could not read analyzer plugin {}: {error}",
                source_path.display()
            )
        })?;
        let checksum = sha256_hex(&bytes);
        let plugin_dir = self
            .install_dir()
            .join(&manifest.plugin_id)
            .join(&manifest.version);
        fs::create_dir_all(&plugin_dir).map_err(|error| {
            format!(
                "Could not create analyzer plugin directory {}: {error}",
                plugin_dir.display()
            )
        })?;
        let installed_path = plugin_dir.join(format!("{checksum}.wasm"));
        fs::write(&installed_path, bytes).map_err(|error| {
            format!(
                "Could not install analyzer plugin {}: {error}",
                installed_path.display()
            )
        })?;

        let mut registry = self.load()?;
        registry.plugins.retain(|plugin| {
            !(plugin.plugin_id == manifest.plugin_id && plugin.version == manifest.version)
        });
        let installed = InstalledAnalyzerPlugin {
            plugin_id: manifest.plugin_id.clone(),
            version: manifest.version.clone(),
            path: installed_path,
            checksum,
            enabled: true,
            installed_at_unix_ms: unix_ms_now(),
            manifest,
        };
        registry.plugins.push(installed.clone());
        self.save(&registry)?;

        Ok(installed)
    }

    pub fn remove_plugin(&self, plugin_id: &str) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
        let mut registry = self.load()?;
        let mut removed = Vec::new();
        registry.plugins.retain(|plugin| {
            if plugin.plugin_id == plugin_id {
                removed.push(plugin.clone());
                false
            } else {
                true
            }
        });

        if removed.is_empty() {
            return Err(format!("Analyzer plugin {plugin_id} is not installed."));
        }

        for plugin in &removed {
            if plugin.path.is_file() {
                fs::remove_file(&plugin.path).map_err(|error| {
                    format!(
                        "Could not remove analyzer plugin file {}: {error}",
                        plugin.path.display()
                    )
                })?;
            }
        }

        self.save(&registry)?;
        Ok(removed)
    }

    pub fn set_plugin_enabled(
        &self,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
        let mut registry = self.load()?;
        let mut updated = Vec::new();

        for plugin in &mut registry.plugins {
            if plugin.plugin_id == plugin_id {
                plugin.enabled = enabled;
                updated.push(plugin.clone());
            }
        }

        if updated.is_empty() {
            return Err(format!("Analyzer plugin {plugin_id} is not installed."));
        }

        self.save(&registry)?;
        Ok(updated)
    }
}

struct PluginRuntime {
    store: Store<()>,
    instance: Instance,
    memory: Memory,
}

impl PluginRuntime {
    fn load(plugin_path: &Path) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, plugin_path)
            .map_err(|error| format!("Could not load WASM plugin: {error}"))?;
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|error| format!("Could not instantiate WASM plugin: {error}"))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| "WASM plugin must export memory.".to_string())?;

        Ok(Self {
            store,
            instance,
            memory,
        })
    }

    fn call_json<T: Serialize>(
        &mut self,
        function_name: &str,
        input: &T,
    ) -> Result<String, String> {
        let input = serde_json::to_vec(input)
            .map_err(|error| format!("Could not serialize plugin input: {error}"))?;
        let alloc = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "peregrine_alloc")
            .map_err(|error| format!("WASM plugin must export peregrine_alloc: {error}"))?;
        let function = self
            .instance
            .get_typed_func::<(i32, i32), i64>(&mut self.store, function_name)
            .map_err(|error| format!("WASM plugin must export {function_name}: {error}"))?;
        let input_ptr = alloc
            .call(&mut self.store, input.len() as i32)
            .map_err(|error| format!("Plugin allocation failed: {error}"))?;

        self.memory
            .write(&mut self.store, input_ptr as usize, &input)
            .map_err(|error| format!("Could not write plugin input memory: {error}"))?;

        let packed = function
            .call(&mut self.store, (input_ptr, input.len() as i32))
            .map_err(|error| format!("Plugin function {function_name} failed: {error}"))?;
        let (output_ptr, output_len) = unpack_plugin_result(packed)?;
        let mut output = vec![0_u8; output_len as usize];

        self.memory
            .read(&mut self.store, output_ptr as usize, &mut output)
            .map_err(|error| format!("Could not read plugin output memory: {error}"))?;

        String::from_utf8(output).map_err(|error| format!("Plugin output is not UTF-8: {error}"))
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

pub fn resolve_plugin_path(package_path: &Path, plugin_path: &Path) -> PathBuf {
    if plugin_path.is_absolute() {
        plugin_path.to_path_buf()
    } else {
        package_path.join(plugin_path)
    }
}

fn unpack_plugin_result(result: i64) -> Result<(u32, u32), String> {
    if result < 0 {
        return Err("Plugin returned a negative result pointer/length pair.".to_string());
    }

    let result = result as u64;
    let ptr = (result >> 32) as u32;
    let len = (result & 0xffff_ffff) as u32;

    Ok((ptr, len))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);

    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }

    output
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn platform_config_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "Could not resolve APPDATA for analyzer plugin registry.".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
            .ok_or_else(|| "Could not resolve HOME for analyzer plugin registry.".to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config_home));
        }

        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config"))
            .ok_or_else(|| "Could not resolve HOME for analyzer plugin registry.".to_string())
    }
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
