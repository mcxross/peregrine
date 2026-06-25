fn main() {
    let mut config = codex_core_plugins::PluginsConfigInput::new(
        codex_compat::ConfigLayerStack::default(),
        true,
        true,
        None,
    );
    // Let's see if we get compile errors
    config.bundled_plugins.push(codex_core_plugins::registry::BundledPlugin {
        name: "test".to_string(),
        directory: std::path::PathBuf::from("test"),
    });
}
