use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{AnalysisContext, Finding, RuleMetric};
use peregrine_types::analysis::{
    RuleConfig, RuleConfigProperty, RuleConfigValueKind, RuleMetadata, RuleSetMetadata, Severity,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAnalyzeInput {
    pub schema_version: u32,
    pub context: AnalysisContext,
    pub config: Value,
    #[serde(default)]
    pub active_rules: Vec<PluginActiveRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub schema_version: u32,
    pub plugin_id: String,
    pub version: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub rulesets: Vec<PluginRuleSetManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuleSetManifest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub rules: Vec<PluginRuleManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuleManifest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default_severity: Option<Severity>,
    #[serde(default)]
    pub config_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginActiveRuleConfig {
    pub ruleset_id: String,
    pub rule_id: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginAnalyzeOutput {
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub metrics: Vec<RuleMetric>,
}

pub fn plugin_manifest_rulesets(manifest: &PluginManifest) -> Vec<RuleSetMetadata> {
    manifest
        .rulesets
        .iter()
        .map(|ruleset| RuleSetMetadata {
            id: ruleset.id.clone(),
            name: ruleset.name.clone().unwrap_or_else(|| ruleset.id.clone()),
            description: ruleset.description.clone().unwrap_or_default(),
            bundled: false,
            plugin_id: Some(manifest.plugin_id.clone()),
            active: true,
            rules: ruleset
                .rules
                .iter()
                .map(|rule| RuleMetadata {
                    id: rule.id.clone(),
                    name: rule.name.clone().unwrap_or_else(|| rule.id.clone()),
                    description: rule.description.clone().unwrap_or_default(),
                    active: true,
                    default_severity: rule.default_severity.clone().unwrap_or(Severity::Warning),
                    configured_severity: None,
                    config_schema: rule
                        .config_keys
                        .iter()
                        .map(|key| RuleConfigProperty {
                            key: key.clone(),
                            value_kind: RuleConfigValueKind::String,
                            description: String::new(),
                            default_value: None,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

pub fn plugin_rule_config_value(config: &RuleConfig) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}
