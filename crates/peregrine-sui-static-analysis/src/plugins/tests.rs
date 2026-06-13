use super::*;
use crate::{AnalysisConfig, Analyzer, Finding, Metric, Severity};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

#[test]
fn wasm_plugin_manifest_is_discovered() {
    let package = move_package();
    let plugin_path = write_wasm_fixture_plugin(package.path(), "plugin.wasm");
    let manifest = AnalysisPluginHost
        .discover_manifest(&plugin_path)
        .expect("manifest");

    assert_eq!(manifest.plugin_id, "fixture-plugin");
    assert_eq!(manifest.rulesets[0].id, "fixture");
}

#[test]
fn wasm_plugin_findings_are_merged() {
    let package = move_package();
    let plugin_path = write_wasm_fixture_plugin(package.path(), "plugin.wasm");
    let mut config = AnalysisConfig::default();
    config.analysis.plugins.paths = vec![plugin_path.strip_prefix(package.path()).unwrap().into()];

    let report = Analyzer::new().analyze_package(package.path(), config);

    assert!(
        report
            .loaded_plugins
            .iter()
            .any(|plugin| plugin == "fixture-plugin")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.rule_id == "FixtureRule")
    );
}

#[test]
fn native_plugin_manifest_is_discovered() {
    let package = move_package();
    let plugin_path = write_native_fixture_plugin(package.path(), "native_fixture");
    let manifest = AnalysisPluginHost
        .discover_manifest(&plugin_path)
        .expect("manifest");

    assert_eq!(manifest.plugin_id, "native-fixture-plugin");
    assert_eq!(manifest.rulesets[0].id, "native_fixture");
}

#[test]
fn native_plugin_findings_are_merged() {
    let package = move_package();
    let plugin_path = write_native_fixture_plugin(package.path(), "native_fixture");
    let mut config = AnalysisConfig::default();
    config.analysis.plugins.paths = vec![plugin_path.strip_prefix(package.path()).unwrap().into()];

    let report = Analyzer::new().analyze_package(package.path(), config);

    assert!(
        report
            .loaded_plugins
            .iter()
            .any(|plugin| plugin == "native-fixture-plugin")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.rule_id == "native_fixture")
    );
}

#[test]
fn plugin_failures_are_reported_as_diagnostics() {
    let package = move_package();
    let mut config = AnalysisConfig::default();
    config.analysis.plugins.paths = vec![PathBuf::from("missing.wasm")];

    let report = Analyzer::new().analyze_package(package.path(), config);

    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source.contains("missing.wasm"))
    );
}

#[test]
fn registry_installs_and_disables_wasm_plugins() {
    let package = move_package();
    let plugin_path = write_wasm_fixture_plugin(package.path(), "plugin.wasm");
    let registry_root = tempfile::tempdir().expect("registry");
    let registry = AnalyzerPluginRegistry::at_root(registry_root.path());
    let installed = registry
        .install_plugin(&plugin_path, &AnalysisPluginHost)
        .expect("install");

    assert_eq!(installed.plugin_id, "fixture-plugin");
    assert!(installed.enabled);
    assert!(installed.path.is_file());
    assert_eq!(registry.enabled_plugin_paths().expect("enabled").len(), 1);

    registry
        .set_plugin_enabled("fixture-plugin", false)
        .expect("disable");

    assert!(
        registry
            .enabled_plugin_paths()
            .expect("enabled after disable")
            .is_empty()
    );
}

#[test]
fn registry_installs_and_disables_native_plugins() {
    let package = move_package();
    let plugin_path = write_native_fixture_plugin(package.path(), "native_registry_fixture");
    let registry_root = tempfile::tempdir().expect("registry");
    let registry = AnalyzerPluginRegistry::at_root(registry_root.path());
    let installed = registry
        .install_plugin(&plugin_path, &AnalysisPluginHost)
        .expect("install");

    assert_eq!(installed.plugin_id, "native-fixture-plugin");
    assert_eq!(installed.runtime, crate::PluginRuntimeKind::Native);
    assert!(installed.enabled);
    assert!(installed.path.is_file());
    assert_eq!(registry.enabled_plugin_paths().expect("enabled").len(), 1);

    registry
        .set_plugin_enabled("native-fixture-plugin", false)
        .expect("disable");

    assert!(
        registry
            .enabled_plugin_paths()
            .expect("enabled after disable")
            .is_empty()
    );
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

fn write_wasm_fixture_plugin(package_path: &Path, name: &str) -> PathBuf {
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

fn write_native_fixture_plugin(package_path: &Path, stem: &str) -> PathBuf {
    let source_path = package_path.join(format!("{stem}.rs"));
    let output_path = package_path.join(format!(
        "{}{stem}.{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_EXTENSION
    ));
    fs::write(&source_path, native_fixture_source()).expect("native fixture source");

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("--crate-type")
        .arg("cdylib")
        .arg("--edition=2021")
        .arg(&source_path)
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("run rustc");

    assert!(
        output.status.success(),
        "native plugin fixture did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    output_path
}

fn native_fixture_source() -> &'static str {
    r##"
#[repr(C)]
pub struct PeregrinePluginBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

fn output(source: &str) -> *mut PeregrinePluginBuffer {
    let mut bytes = source.as_bytes().to_vec();
    let ptr = bytes.as_mut_ptr();
    let len = bytes.len();
    std::mem::forget(bytes);
    Box::into_raw(Box::new(PeregrinePluginBuffer { ptr, len }))
}

#[no_mangle]
pub extern "C" fn peregrine_plugin_manifest(
    _input_ptr: *const u8,
    _input_len: usize,
) -> *mut PeregrinePluginBuffer {
    output(r#"{"schemaVersion":1,"pluginId":"native-fixture-plugin","version":"0.1.0","rulesets":[{"id":"native_fixture","rules":[{"id":"native_fixture","configKeys":[]}]}]}"#)
}

#[no_mangle]
pub extern "C" fn peregrine_analyze(
    _input_ptr: *const u8,
    _input_len: usize,
) -> *mut PeregrinePluginBuffer {
    output(r#"{"findings":[{"ruleId":"native_fixture","rulesetId":"native_fixture","severity":"warning","message":"native fixture finding","file":"sources/m.move","span":null,"metric":null}],"metrics":[]}"#)
}

#[no_mangle]
pub unsafe extern "C" fn peregrine_plugin_free(buffer: *mut PeregrinePluginBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = Box::from_raw(buffer);
    if !buffer.ptr.is_null() && buffer.len > 0 {
        let _ = Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.len);
    }
}
"##
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
