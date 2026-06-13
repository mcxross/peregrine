mod host;
mod manifest;
mod registry;

#[cfg(test)]
mod tests;

pub use peregrine_plugins::{PluginManifestInput, resolve_plugin_path};

pub use host::{AnalysisPluginHost, PluginAnalysisReport};
pub use manifest::{
    PluginActiveRuleConfig, PluginAnalyzeInput, PluginAnalyzeOutput, PluginManifest,
    PluginRuleManifest, PluginRuleSetManifest, plugin_manifest_rulesets, plugin_rule_config_value,
};
pub use registry::{AnalyzerPluginRegistry, AnalyzerPluginRegistryFile, InstalledAnalyzerPlugin};
