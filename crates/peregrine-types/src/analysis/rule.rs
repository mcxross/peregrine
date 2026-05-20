use crate::{AnalysisContext, Finding, RuleConfig, RuleMetric, Severity};
use serde::{Deserialize, Serialize};

pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            id: self.id().to_string(),
            name: self.id().to_string(),
            description: String::new(),
            active: true,
            default_severity: Severity::Warning,
            configured_severity: None,
            config_schema: Vec::new(),
        }
    }

    fn analyze(&self, context: &AnalysisContext, config: &RuleConfig) -> RuleOutcome;
}

pub trait RuleSet: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> RuleSetMetadata {
        RuleSetMetadata {
            id: self.id().to_string(),
            name: self.id().to_string(),
            description: String::new(),
            bundled: false,
            plugin_id: None,
            active: true,
            rules: self
                .rules()
                .into_iter()
                .map(|rule| rule.metadata())
                .collect(),
        }
    }

    fn rules(&self) -> Vec<Box<dyn Rule>>;
}

pub trait RuleSetProvider: Send + Sync {
    fn rule_sets(&self) -> Vec<Box<dyn RuleSet>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub bundled: bool,
    pub plugin_id: Option<String>,
    pub active: bool,
    pub rules: Vec<RuleMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub default_severity: Severity,
    pub configured_severity: Option<Severity>,
    pub config_schema: Vec<RuleConfigProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleConfigProperty {
    pub key: String,
    pub value_kind: RuleConfigValueKind,
    pub description: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuleConfigValueKind {
    Boolean,
    Integer,
    String,
    Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRuleCatalog {
    pub rulesets: Vec<RuleSetMetadata>,
    pub loaded_plugins: Vec<String>,
    pub diagnostics: Vec<crate::AnalysisDiagnostic>,
}

#[derive(Debug, Default)]
pub struct RuleOutcome {
    pub findings: Vec<Finding>,
    pub metrics: Vec<RuleMetric>,
}
