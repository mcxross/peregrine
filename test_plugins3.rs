fn main() {
    let home = std::path::PathBuf::from("/tmp");
    let pm = codex_core_plugins::PluginsManager::new(home);
    pm.non_existent_method();
}
