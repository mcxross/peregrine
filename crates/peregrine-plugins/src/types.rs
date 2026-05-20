use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub const PLUGIN_SCHEMA_VERSION: u32 = 1;
pub(crate) const REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginKind(String);

impl PluginKind {
    pub const STATIC_ANALYSIS: &'static str = "static_analysis";
    pub const DYNAMIC_ANALYSIS: &'static str = "dynamic_analysis";

    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        validate_component("plugin kind", &value)?;
        Ok(Self(value))
    }

    pub fn static_analysis() -> Self {
        Self(Self::STATIC_ANALYSIS.to_string())
    }

    pub fn dynamic_analysis() -> Self {
        Self(Self::DYNAMIC_ANALYSIS.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntimeKind {
    Wasm,
    #[serde(alias = "dylib", alias = "cdylib")]
    Native,
}

impl PluginRuntimeKind {
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::Native => std::env::consts::DLL_EXTENSION,
        }
    }

    pub fn from_path(path: &Path) -> Result<Self, String> {
        match path
            .extension()
            .and_then(OsStr::to_str)
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref()
        {
            Some("wasm") => Ok(Self::Wasm),
            Some("dylib" | "so" | "dll") => Ok(Self::Native),
            Some(extension) => Err(format!(
                "Could not infer plugin runtime from extension `{extension}` for {}.",
                path.display()
            )),
            None => Err(format!(
                "Could not infer plugin runtime because {} has no extension.",
                path.display()
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestInput {
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallManifest {
    pub plugin_id: String,
    pub version: String,
    pub kind: PluginKind,
    pub runtime: PluginRuntimeKind,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub manifest: Value,
}

impl PluginInstallManifest {
    pub fn validate(&self) -> Result<(), String> {
        validate_component("plugin id", &self.plugin_id)?;
        validate_component("plugin version", &self.version)?;

        if self.manifest.is_null() {
            return Err("Plugin manifest cannot be null.".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub plugin_id: String,
    pub version: String,
    pub kind: PluginKind,
    pub runtime: PluginRuntimeKind,
    pub path: PathBuf,
    pub checksum: String,
    pub enabled: bool,
    pub installed_at_unix_ms: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub manifest: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistryFile {
    pub version: u32,
    pub plugins: Vec<InstalledPlugin>,
}

impl Default for PluginRegistryFile {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            plugins: Vec::new(),
        }
    }
}

pub(crate) fn validate_component(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} cannot be empty."));
    }

    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(format!(
            "{label} `{value}` may only contain ASCII letters, numbers, dots, dashes, and underscores."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_kind_is_inferred_from_plugin_extension() {
        assert_eq!(
            PluginRuntimeKind::from_path(Path::new("plugin.wasm")).expect("wasm"),
            PluginRuntimeKind::Wasm
        );
        assert_eq!(
            PluginRuntimeKind::from_path(Path::new("plugin.dylib")).expect("dylib"),
            PluginRuntimeKind::Native
        );
        assert_eq!(
            PluginRuntimeKind::from_path(Path::new("plugin.so")).expect("so"),
            PluginRuntimeKind::Native
        );
        assert_eq!(
            PluginRuntimeKind::from_path(Path::new("plugin.dll")).expect("dll"),
            PluginRuntimeKind::Native
        );
    }
}
