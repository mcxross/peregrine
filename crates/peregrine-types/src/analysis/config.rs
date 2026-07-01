#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use super::model::Severity;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnalysisConfig {
    pub analysis: AnalysisSection,
}

impl AnalysisConfig {
    pub fn load_from_package(package_path: impl AsRef<Path>) -> Result<Self, String> {
        let config_path = package_path.as_ref().join("peregrine.toml");

        if !config_path.is_file() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&config_path)
            .map_err(|error| format!("Could not read {}: {error}", config_path.display()))?;
        toml::from_str::<Self>(&contents)
            .map(AnalysisConfig::with_defaults)
            .map_err(|error| format!("Could not parse {}: {error}", config_path.display()))
    }

    pub fn save_to_package(&self, package_path: impl AsRef<Path>) -> Result<(), String> {
        let config_path = package_path.as_ref().join("peregrine.toml");
        let contents = toml::to_string_pretty(self)
            .map_err(|error| format!("Could not serialize {}: {error}", config_path.display()))?;

        fs::write(&config_path, contents)
            .map_err(|error| format!("Could not write {}: {error}", config_path.display()))
    }

    pub fn with_defaults(mut self) -> Self {
        let defaults = Self::default();

        if self.analysis.include.is_empty() {
            self.analysis.include = defaults.analysis.include;
        }

        if self.analysis.exclude.is_empty() {
            self.analysis.exclude = defaults.analysis.exclude;
        }

        for (ruleset_id, default_ruleset) in defaults.analysis.rulesets {
            let ruleset = self
                .analysis
                .rulesets
                .entry(ruleset_id)
                .or_insert_with(|| default_ruleset.clone());

            if ruleset.active.is_none() {
                ruleset.active = default_ruleset.active;
            }

            if ruleset.severity.is_none() {
                ruleset.severity = default_ruleset.severity;
            }

            if ruleset.threshold.is_none() {
                ruleset.threshold = default_ruleset.threshold;
            }

            if ruleset.entry_threshold.is_none() {
                ruleset.entry_threshold = default_ruleset.entry_threshold;
            }

            for (rule_id, default_rule) in default_ruleset.rules {
                let rule = ruleset
                    .rules
                    .entry(rule_id)
                    .or_insert_with(|| default_rule.clone());

                if rule.active.is_none() {
                    rule.active = default_rule.active;
                }

                if rule.severity.is_none() {
                    rule.severity = default_rule.severity;
                }

                if rule.threshold.is_none() {
                    rule.threshold = default_rule.threshold;
                }

                if rule.entry_threshold.is_none() {
                    rule.entry_threshold = default_rule.entry_threshold;
                }
            }
        }

        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisSection {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub plugins: PluginConfig,
    pub rulesets: BTreeMap<String, RuleSetConfig>,
}

impl Default for AnalysisSection {
    fn default() -> Self {
        let mut rulesets = BTreeMap::new();
        let mut complexity_rules = BTreeMap::new();

        complexity_rules.insert(
            "function_complexity".to_string(),
            RuleConfig {
                active: Some(true),
                severity: Some(Severity::Warning),
                threshold: Some(15),
                entry_threshold: Some(12),
                extra: BTreeMap::new(),
            },
        );
        complexity_rules.insert(
            "module_complexity".to_string(),
            RuleConfig {
                active: Some(true),
                severity: Some(Severity::Warning),
                threshold: Some(80),
                entry_threshold: None,
                extra: BTreeMap::new(),
            },
        );
        rulesets.insert(
            "complexity".to_string(),
            RuleSetConfig {
                active: Some(true),
                severity: None,
                threshold: None,
                entry_threshold: None,
                rules: complexity_rules,
            },
        );
        for rule_id in SUI_RULESET_IDS {
            rulesets.insert(
                rule_id.to_string(),
                RuleSetConfig {
                    active: Some(true),
                    severity: None,
                    threshold: None,
                    entry_threshold: None,
                    rules: BTreeMap::new(),
                },
            );
        }

        Self {
            include: vec!["sources/**/*.move".to_string()],
            exclude: vec!["build/**".to_string(), ".move/**".to_string()],
            plugins: PluginConfig::default(),
            rulesets,
        }
    }
}

const SUI_RULESET_IDS: &[&str] = &[
    "bool_judgement",
    "infinite_loop",
    "precision_loss",
    "type_conversion",
    "unchecked_return",
    "unused_const",
    "unused_private_function",
    "unused_struct",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginConfig {
    pub use_global: bool,
    pub paths: Vec<PathBuf>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            use_global: true,
            paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuleSetConfig {
    pub active: Option<bool>,
    pub severity: Option<Severity>,
    pub threshold: Option<u32>,
    pub entry_threshold: Option<u32>,
    #[serde(flatten)]
    pub rules: BTreeMap<String, RuleConfig>,
}

impl RuleSetConfig {
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }

    pub fn rule_config(&self, rule_id: &str) -> RuleConfig {
        let mut config = self.rules.get(rule_id).cloned().unwrap_or_default();

        if config.severity.is_none() {
            config.severity = self.severity.clone();
        }

        if config.threshold.is_none() {
            config.threshold = self.threshold;
        }

        if config.entry_threshold.is_none() {
            config.entry_threshold = self.entry_threshold;
        }

        config
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuleConfig {
    pub active: Option<bool>,
    pub severity: Option<Severity>,
    pub threshold: Option<u32>,
    pub entry_threshold: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl RuleConfig {
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }

    pub fn severity_or(&self, default: Severity) -> Severity {
        self.severity.clone().unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_config_defaults_to_global_plugins() {
        let config = toml::from_str::<AnalysisConfig>("[analysis.plugins]\npaths = []\n")
            .unwrap()
            .with_defaults();

        assert!(config.analysis.plugins.use_global);
        assert!(config.analysis.plugins.paths.is_empty());
    }

    #[test]
    fn rule_config_accepts_severity_override() {
        let config = toml::from_str::<AnalysisConfig>(
            r#"
[analysis.rulesets.complexity.function_complexity]
severity = "error"
"#,
        )
        .unwrap()
        .with_defaults();

        let severity = config
            .analysis
            .rulesets
            .get("complexity")
            .unwrap()
            .rules
            .get("function_complexity")
            .unwrap()
            .severity
            .clone();

        assert_eq!(severity, Some(Severity::Error));
    }

    #[test]
    fn single_rule_ruleset_accepts_direct_rule_config() {
        let config = toml::from_str::<AnalysisConfig>(
            r#"
[analysis.rulesets.unchecked_return]
severity = "error"
"#,
        )
        .unwrap()
        .with_defaults();

        let severity = config
            .analysis
            .rulesets
            .get("unchecked_return")
            .unwrap()
            .rule_config("unchecked_return")
            .severity;

        assert_eq!(severity, Some(Severity::Error));
    }
}
