use std::path::{Path, PathBuf};

use peregrine_plugins::{
    InstalledPlugin, PluginInstallManifest, PluginKind, PluginRegistry, PluginRegistryFile,
    PluginRuntimeKind,
};

use super::AnalysisPluginHost;

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

    #[allow(clippy::should_implement_trait)]
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
        host: &AnalysisPluginHost,
    ) -> Result<InstalledAnalyzerPlugin, String> {
        let source_path = source_path.as_ref();
        let manifest = host.discover_manifest(source_path)?;
        let runtime = PluginRuntimeKind::from_path(source_path)?;
        let install_manifest = PluginInstallManifest {
            plugin_id: manifest.plugin_id.clone(),
            version: manifest.version.clone(),
            kind: PluginKind::static_analysis(),
            runtime,
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
