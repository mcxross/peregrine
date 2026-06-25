fn main() {
    let mut config = codex_core_plugins::PluginsConfigInput::new(
        codex_compat::ConfigLayerStack::default(),
        true,
        true,
        None,
    );
    println!("{:?}", config);
}
