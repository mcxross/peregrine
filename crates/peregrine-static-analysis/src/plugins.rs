mod host;
mod manifest;
mod registry;

#[cfg(test)]
mod tests;

pub use peregrine_plugins::{resolve_plugin_path, PluginManifestInput};

pub use host::{AnalysisPluginHost, PluginAnalysisReport};
pub use manifest::{
    plugin_manifest_rulesets, plugin_rule_config_value, PluginActiveRuleConfig, PluginAnalyzeInput,
    PluginAnalyzeOutput, PluginManifest, PluginRuleManifest, PluginRuleSetManifest,
};
pub use registry::{AnalyzerPluginRegistry, AnalyzerPluginRegistryFile, InstalledAnalyzerPlugin};
