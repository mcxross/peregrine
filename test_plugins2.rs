fn main() {
    let home = std::path::PathBuf::from("/tmp");
    let mut pm = codex_core_plugins::PluginsManager::new(home);
}
