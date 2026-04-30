use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use wasmtime::{Engine, Instance, Memory, Module, Store};

use crate::model::{AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, RuleMetric};

pub const PLUGIN_SCHEMA_VERSION: u32 = 1;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub schema_version: u32,
    pub plugin_id: String,
    pub version: String,
    pub rulesets: Vec<PluginRuleSetManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuleSetManifest {
    pub id: String,
    pub rules: Vec<PluginRuleManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuleManifest {
    pub id: String,
    #[serde(default)]
    pub config_keys: Vec<String>,
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

            match self.analyze_plugin(context, &plugin_path) {
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

    fn analyze_plugin(
        &self,
        context: &AnalysisContext,
        plugin_path: &Path,
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
        };
        let analyze_output = runtime.call_json("peregrine_analyze", &analyze_input)?;
        let output: PluginAnalyzeOutput = serde_json::from_str(&analyze_output)
            .map_err(|error| format!("Plugin analysis output is not valid JSON: {error}"))?;

        Ok(PluginAnalysisReport {
            plugin_id: manifest.plugin_id,
            findings: output.findings,
            metrics: output.metrics,
        })
    }
}

struct PluginAnalysisReport {
    plugin_id: String,
    findings: Vec<Finding>,
    metrics: Vec<RuleMetric>,
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

fn resolve_plugin_path(package_path: &Path, plugin_path: &Path) -> PathBuf {
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
