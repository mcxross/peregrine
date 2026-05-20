use crate::{
    InstalledPlugin, PluginInstallManifest, PluginKind, PluginRegistryFile, REGISTRY_VERSION,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const APP_CONFIG_DIR_NAME: &str = "xyz.mcxross.peregrine";
const PLUGIN_REGISTRY_FILE: &str = "plugins.json";
const PLUGIN_INSTALL_DIR: &str = "plugins";

#[derive(Debug, Clone)]
pub struct PluginRegistry {
    root: PathBuf,
}

impl PluginRegistry {
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

    pub fn load(&self) -> Result<PluginRegistryFile, String> {
        let path = self.registry_path();

        if !path.is_file() {
            return Ok(PluginRegistryFile::default());
        }

        let contents = fs::read_to_string(&path).map_err(|error| {
            format!("Could not read plugin registry {}: {error}", path.display())
        })?;
        let mut registry =
            serde_json::from_str::<PluginRegistryFile>(&contents).map_err(|error| {
                format!(
                    "Could not parse plugin registry {}: {error}",
                    path.display()
                )
            })?;

        if registry.version != REGISTRY_VERSION {
            return Err(format!(
                "Unsupported plugin registry version {}. Expected {}.",
                registry.version, REGISTRY_VERSION
            ));
        }

        sort_plugins(&mut registry.plugins);
        Ok(registry)
    }

    pub fn save(&self, registry: &PluginRegistryFile) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|error| {
            format!(
                "Could not create plugin registry directory {}: {error}",
                self.root.display()
            )
        })?;
        let contents = serde_json::to_string_pretty(registry)
            .map_err(|error| format!("Could not serialize plugin registry: {error}"))?;
        fs::write(self.registry_path(), contents).map_err(|error| {
            format!(
                "Could not write plugin registry {}: {error}",
                self.registry_path().display()
            )
        })
    }

    pub fn list_plugins(&self) -> Result<Vec<InstalledPlugin>, String> {
        Ok(self.load()?.plugins)
    }

    pub fn list_plugins_by_kind(&self, kind: &PluginKind) -> Result<Vec<InstalledPlugin>, String> {
        Ok(self
            .load()?
            .plugins
            .into_iter()
            .filter(|plugin| &plugin.kind == kind)
            .collect())
    }

    pub fn enabled_plugin_paths_for_kind(&self, kind: &PluginKind) -> Result<Vec<PathBuf>, String> {
        Ok(self
            .list_plugins_by_kind(kind)?
            .into_iter()
            .filter(|plugin| plugin.enabled)
            .map(|plugin| plugin.path)
            .collect())
    }

    pub fn install_plugin(
        &self,
        source_path: impl AsRef<Path>,
        manifest: PluginInstallManifest,
    ) -> Result<InstalledPlugin, String> {
        manifest.validate()?;

        let source_path = source_path.as_ref();
        let bytes = fs::read(source_path)
            .map_err(|error| format!("Could not read plugin {}: {error}", source_path.display()))?;
        let checksum = sha256_hex(&bytes);
        let plugin_dir = self
            .install_dir()
            .join(manifest.kind.as_str())
            .join(&manifest.plugin_id)
            .join(&manifest.version);
        fs::create_dir_all(&plugin_dir).map_err(|error| {
            format!(
                "Could not create plugin directory {}: {error}",
                plugin_dir.display()
            )
        })?;
        let installed_path =
            plugin_dir.join(format!("{checksum}.{}", manifest.runtime.file_extension()));
        fs::write(&installed_path, bytes).map_err(|error| {
            format!(
                "Could not install plugin {}: {error}",
                installed_path.display()
            )
        })?;

        let mut registry = self.load()?;
        registry.plugins.retain(|plugin| {
            !(plugin.kind == manifest.kind
                && plugin.plugin_id == manifest.plugin_id
                && plugin.version == manifest.version)
        });
        let installed = InstalledPlugin {
            plugin_id: manifest.plugin_id,
            version: manifest.version,
            kind: manifest.kind,
            runtime: manifest.runtime,
            path: installed_path,
            checksum,
            enabled: true,
            installed_at_unix_ms: unix_ms_now(),
            name: manifest.name,
            description: manifest.description,
            manifest: manifest.manifest,
        };

        registry.plugins.push(installed.clone());
        sort_plugins(&mut registry.plugins);
        self.save(&registry)?;

        Ok(installed)
    }

    pub fn remove_plugin(
        &self,
        kind: &PluginKind,
        plugin_id: &str,
    ) -> Result<Vec<InstalledPlugin>, String> {
        let mut registry = self.load()?;
        let mut removed = Vec::new();

        registry.plugins.retain(|plugin| {
            if &plugin.kind == kind && plugin.plugin_id == plugin_id {
                removed.push(plugin.clone());
                false
            } else {
                true
            }
        });

        if removed.is_empty() {
            return Err(format!(
                "{} plugin {plugin_id} is not installed.",
                kind.as_str()
            ));
        }

        for plugin in &removed {
            if plugin.path.is_file() {
                fs::remove_file(&plugin.path).map_err(|error| {
                    format!(
                        "Could not remove plugin file {}: {error}",
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
        kind: &PluginKind,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<Vec<InstalledPlugin>, String> {
        let mut registry = self.load()?;
        let mut updated = Vec::new();

        for plugin in &mut registry.plugins {
            if &plugin.kind == kind && plugin.plugin_id == plugin_id {
                plugin.enabled = enabled;
                updated.push(plugin.clone());
            }
        }

        if updated.is_empty() {
            return Err(format!(
                "{} plugin {plugin_id} is not installed.",
                kind.as_str()
            ));
        }

        self.save(&registry)?;
        Ok(updated)
    }
}

fn sort_plugins(plugins: &mut [InstalledPlugin]) {
    plugins.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.plugin_id.cmp(&right.plugin_id))
            .then(left.version.cmp(&right.version))
    });
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
            .ok_or_else(|| "Could not resolve APPDATA for plugin registry.".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
            .ok_or_else(|| "Could not resolve HOME for plugin registry.".to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config_home));
        }

        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config"))
            .ok_or_else(|| "Could not resolve HOME for plugin registry.".to_string())
    }
}

#[cfg(test)]
mod tests;
