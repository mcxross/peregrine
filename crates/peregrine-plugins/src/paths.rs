use std::path::{Path, PathBuf};

pub fn resolve_plugin_path(package_path: &Path, plugin_path: &Path) -> PathBuf {
    if plugin_path.is_absolute() {
        plugin_path.to_path_buf()
    } else {
        package_path.join(plugin_path)
    }
}
