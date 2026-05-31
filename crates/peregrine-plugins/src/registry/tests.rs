use super::*;
use crate::PluginRuntimeKind;
use std::{ffi::OsStr, fs};

#[test]
fn registry_filters_enabled_plugins_by_kind() {
    let root = tempfile::tempdir().expect("registry");
    let source = root.path().join("plugin.wasm");
    fs::write(&source, b"wasm").expect("plugin file");
    let registry = PluginRegistry::at_root(root.path());

    registry
        .install_plugin(
            &source,
            PluginInstallManifest {
                plugin_id: "fixture".to_string(),
                version: "0.1.0".to_string(),
                kind: PluginKind::static_analysis(),
                runtime: PluginRuntimeKind::Wasm,
                name: Some("Fixture".to_string()),
                description: None,
                manifest: serde_json::json!({ "pluginId": "fixture" }),
            },
        )
        .expect("install");

    assert_eq!(
        registry
            .enabled_plugin_paths_for_kind(&PluginKind::static_analysis())
            .expect("enabled static")
            .len(),
        1
    );
    assert!(
        registry
            .enabled_plugin_paths_for_kind(&PluginKind::dynamic_analysis())
            .expect("enabled dynamic")
            .is_empty()
    );
}

#[test]
fn registry_installs_native_plugins_with_platform_extension() {
    let root = tempfile::tempdir().expect("registry");
    let source = root.path().join(format!(
        "{}fixture.{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_EXTENSION
    ));
    fs::write(&source, b"native").expect("plugin file");
    let registry = PluginRegistry::at_root(root.path());

    let installed = registry
        .install_plugin(
            &source,
            PluginInstallManifest {
                plugin_id: "fixture-native".to_string(),
                version: "0.1.0".to_string(),
                kind: PluginKind::dynamic_analysis(),
                runtime: PluginRuntimeKind::Native,
                name: Some("Native Fixture".to_string()),
                description: None,
                manifest: serde_json::json!({ "pluginId": "fixture-native" }),
            },
        )
        .expect("install");

    assert_eq!(installed.runtime, PluginRuntimeKind::Native);
    assert_eq!(
        installed.path.extension().and_then(OsStr::to_str),
        Some(std::env::consts::DLL_EXTENSION)
    );
    assert_eq!(
        registry
            .enabled_plugin_paths_for_kind(&PluginKind::dynamic_analysis())
            .expect("enabled dynamic")
            .len(),
        1
    );
}
