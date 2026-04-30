use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisConfig {
    pub analysis: AnalysisSection,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            analysis: AnalysisSection::default(),
        }
    }
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
            .map(|config| config.with_defaults())
            .map_err(|error| format!("Could not parse {}: {error}", config_path.display()))
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

            for (rule_id, default_rule) in default_ruleset.rules {
                let rule = ruleset
                    .rules
                    .entry(rule_id)
                    .or_insert_with(|| default_rule.clone());

                if rule.active.is_none() {
                    rule.active = default_rule.active;
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
            "FunctionComplexity".to_string(),
            RuleConfig {
                active: Some(true),
                threshold: Some(15),
                entry_threshold: Some(12),
                extra: BTreeMap::new(),
            },
        );
        complexity_rules.insert(
            "ModuleComplexity".to_string(),
            RuleConfig {
                active: Some(true),
                threshold: Some(80),
                entry_threshold: None,
                extra: BTreeMap::new(),
            },
        );
        rulesets.insert(
            "complexity".to_string(),
            RuleSetConfig {
                active: Some(true),
                rules: complexity_rules,
            },
        );

        Self {
            include: vec!["sources/**/*.move".to_string()],
            exclude: vec!["build/**".to_string(), ".move/**".to_string()],
            plugins: PluginConfig::default(),
            rulesets,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PluginConfig {
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuleSetConfig {
    pub active: Option<bool>,
    #[serde(flatten)]
    pub rules: BTreeMap<String, RuleConfig>,
}

impl RuleSetConfig {
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }

    pub fn rule_config(&self, rule_id: &str) -> RuleConfig {
        self.rules.get(rule_id).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuleConfig {
    pub active: Option<bool>,
    pub threshold: Option<u32>,
    pub entry_threshold: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl RuleConfig {
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }
}
