use std::collections::BTreeMap;

use crate::{
    analyzer::{Rule, RuleOutcome, RuleSet, RuleSetProvider},
    config::RuleConfig,
    model::{AnalysisContext, Finding, Metric, ParsedFunction, ParsedModule, RuleMetric, Severity},
};

const RULESET_ID: &str = "complexity";
const FUNCTION_RULE_ID: &str = "FunctionComplexity";
const MODULE_RULE_ID: &str = "ModuleComplexity";
const DEFAULT_FUNCTION_THRESHOLD: u32 = 15;
const DEFAULT_ENTRY_FUNCTION_THRESHOLD: u32 = 12;
const DEFAULT_MODULE_THRESHOLD: u32 = 80;

pub struct ComplexityRuleSetProvider;

impl RuleSetProvider for ComplexityRuleSetProvider {
    fn rule_sets(&self) -> Vec<Box<dyn RuleSet>> {
        vec![Box::new(ComplexityRuleSet)]
    }
}

pub struct ComplexityRuleSet;

impl RuleSet for ComplexityRuleSet {
    fn id(&self) -> &'static str {
        RULESET_ID
    }

    fn rules(&self) -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(FunctionComplexityRule),
            Box::new(ModuleComplexityRule),
        ]
    }
}

pub struct FunctionComplexityRule;

impl Rule for FunctionComplexityRule {
    fn id(&self) -> &'static str {
        FUNCTION_RULE_ID
    }

    fn analyze(&self, context: &AnalysisContext, config: &RuleConfig) -> RuleOutcome {
        let threshold = config.threshold.unwrap_or(DEFAULT_FUNCTION_THRESHOLD);
        let entry_threshold = config
            .entry_threshold
            .unwrap_or(DEFAULT_ENTRY_FUNCTION_THRESHOLD);
        let mut outcome = RuleOutcome::default();

        for module in &context.modules {
            for function in &module.functions {
                let score = function_complexity_score(function);
                let active_threshold = if function.is_transaction_callable {
                    entry_threshold
                } else {
                    threshold
                };
                let target = format!("{}::{}", module.name, function.name);
                let metric = Metric {
                    name: "complexity".to_string(),
                    value: score,
                    threshold: Some(active_threshold),
                };

                outcome.metrics.push(RuleMetric {
                    ruleset_id: RULESET_ID.to_string(),
                    rule_id: FUNCTION_RULE_ID.to_string(),
                    target: target.clone(),
                    file: Some(function.file.clone()),
                    span: function.span,
                    metric: metric.clone(),
                });

                if score > active_threshold {
                    outcome.findings.push(Finding {
                        rule_id: FUNCTION_RULE_ID.to_string(),
                        ruleset_id: RULESET_ID.to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "{target} has complexity {score}, above the configured threshold {active_threshold}."
                        ),
                        file: function.file.clone(),
                        span: function.span,
                        metric: Some(metric),
                    });
                }
            }
        }

        outcome
    }
}

pub struct ModuleComplexityRule;

impl Rule for ModuleComplexityRule {
    fn id(&self) -> &'static str {
        MODULE_RULE_ID
    }

    fn analyze(&self, context: &AnalysisContext, config: &RuleConfig) -> RuleOutcome {
        let threshold = config.threshold.unwrap_or(DEFAULT_MODULE_THRESHOLD);
        let mut outcome = RuleOutcome::default();

        for module in &context.modules {
            let score = module_complexity_score(module);
            let metric = Metric {
                name: "complexity".to_string(),
                value: score,
                threshold: Some(threshold),
            };

            outcome.metrics.push(RuleMetric {
                ruleset_id: RULESET_ID.to_string(),
                rule_id: MODULE_RULE_ID.to_string(),
                target: module.name.clone(),
                file: Some(module.file.clone()),
                span: None,
                metric: metric.clone(),
            });

            if score > threshold {
                outcome.findings.push(Finding {
                    rule_id: MODULE_RULE_ID.to_string(),
                    ruleset_id: RULESET_ID.to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "{} has aggregate complexity {score}, above the configured threshold {threshold}.",
                        module.name
                    ),
                    file: module.file.clone(),
                    span: None,
                    metric: Some(metric),
                });
            }
        }

        outcome
    }
}

pub fn function_complexity_score(function: &ParsedFunction) -> u32 {
    let mut score = 1;
    let source = sanitize_source(&function.body);
    let source = source.as_str();

    score += count_words(source, &["if", "while", "loop", "match"]);
    score += count_phrase(source, "assert!");
    score += count_word(source, "abort");
    score += count_phrase(source, "&&");
    score += count_phrase(source, "||");
    score += function.type_parameter_count;

    if function.is_transaction_callable || function.is_entry {
        score += 2;
    }

    for operation in [
        "transfer::",
        "share_object",
        "freeze_object",
        "dynamic_field",
        "table::",
        "bag::",
        "object::new",
    ] {
        score += count_phrase(source, operation);
    }

    score
}

pub fn module_complexity_score(module: &ParsedModule) -> u32 {
    module.functions.iter().map(function_complexity_score).sum()
}

pub fn module_complexity_scores(context: &AnalysisContext) -> BTreeMap<String, u32> {
    context
        .modules
        .iter()
        .map(|module| (module.name.clone(), module_complexity_score(module)))
        .collect()
}

fn count_words(source: &str, words: &[&str]) -> u32 {
    words.iter().map(|word| count_word(source, word)).sum()
}

fn count_word(source: &str, word: &str) -> u32 {
    source
        .match_indices(word)
        .filter(|(index, _)| {
            let before = source[..*index].chars().next_back();
            let after = source[*index + word.len()..].chars().next();

            before != Some('#')
                && before.is_none_or(|character| !is_identifier_character(character))
                && after.is_none_or(|character| !is_identifier_character(character))
        })
        .count() as u32
}

fn count_phrase(source: &str, phrase: &str) -> u32 {
    source.match_indices(phrase).count() as u32
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn sanitize_source(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        LineComment,
        BlockComment,
        String { escaped: bool },
    }

    let bytes = source.as_bytes();
    let mut sanitized = Vec::with_capacity(bytes.len());
    let mut state = State::Normal;
    let mut index = 0_usize;

    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();

        match state {
            State::Normal if byte == b'/' && next == Some(b'/') => {
                sanitized.extend_from_slice(b"  ");
                state = State::LineComment;
                index += 2;
            }
            State::Normal if byte == b'/' && next == Some(b'*') => {
                sanitized.extend_from_slice(b"  ");
                state = State::BlockComment;
                index += 2;
            }
            State::Normal if byte == b'"' => {
                sanitized.push(byte);
                state = State::String { escaped: false };
                index += 1;
            }
            State::Normal => {
                sanitized.push(byte);
                index += 1;
            }
            State::LineComment if byte == b'\n' => {
                sanitized.push(byte);
                state = State::Normal;
                index += 1;
            }
            State::LineComment => {
                sanitized.push(b' ');
                index += 1;
            }
            State::BlockComment if byte == b'*' && next == Some(b'/') => {
                sanitized.extend_from_slice(b"  ");
                state = State::Normal;
                index += 2;
            }
            State::BlockComment => {
                sanitized.push(if byte == b'\n' { b'\n' } else { b' ' });
                index += 1;
            }
            State::String { escaped: true } => {
                sanitized.push(if byte == b'\n' { b'\n' } else { b' ' });
                state = State::String { escaped: false };
                index += 1;
            }
            State::String { escaped: false } if byte == b'\\' => {
                sanitized.push(b' ');
                state = State::String { escaped: true };
                index += 1;
            }
            State::String { escaped: false } if byte == b'"' => {
                sanitized.push(byte);
                state = State::Normal;
                index += 1;
            }
            State::String { escaped: false } => {
                sanitized.push(if byte == b'\n' { b'\n' } else { b' ' });
                index += 1;
            }
        }
    }

    String::from_utf8(sanitized).unwrap_or_else(|_| source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{AnalysisConfig, RuleSetConfig},
        Analyzer,
    };
    use std::{collections::BTreeMap, fs};
    use tempfile::TempDir;

    #[test]
    fn simple_function_has_base_complexity() {
        let package = move_package(
            r#"
module demo::m {
    fun id(): u64 { 1 }
}
"#,
        );
        let report = Analyzer::new().analyze_package(package.path(), AnalysisConfig::default());
        let metric = report
            .metrics
            .iter()
            .find(|metric| metric.rule_id == FUNCTION_RULE_ID && metric.target == "m::id")
            .expect("function metric");

        assert_eq!(metric.metric.value, 1);
    }

    #[test]
    fn counts_move_specific_complexity() {
        let package = move_package(
            r#"
module demo::m {
    public entry fun mutate<T>(flag: bool, ctx: &mut TxContext) {
        if (flag && true) {
            assert!(flag, 0);
            transfer::share_object(object::new(ctx));
        } else if (!flag || false) {
            loop { abort 1 }
        };
    }
}
"#,
        );
        let report = Analyzer::new().analyze_package(package.path(), AnalysisConfig::default());
        let metric = report
            .metrics
            .iter()
            .find(|metric| metric.rule_id == FUNCTION_RULE_ID && metric.target == "m::mutate")
            .expect("function metric");

        assert_eq!(metric.metric.value, 14);
    }

    #[test]
    fn modern_module_label_parses_without_module_block_and_ignores_method_aliases() {
        let package = move_package(
            r#"
module demo::modern;

public struct Counter has copy, drop { value: u64 }
public use fun value as Counter.value;

public macro fun inspect<T>(value: T, f: |T|) {
    f(value)
}

public entry fun run(flag: bool) {
    if (flag) {
        assert!(flag, 0)
    }
}
"#,
        );
        let report = Analyzer::new().analyze_package(package.path(), AnalysisConfig::default());
        let targets = report
            .metrics
            .iter()
            .filter(|metric| metric.rule_id == FUNCTION_RULE_ID)
            .map(|metric| metric.target.as_str())
            .collect::<Vec<_>>();

        assert!(targets.contains(&"modern::inspect"));
        assert!(targets.contains(&"modern::run"));
        assert!(!targets.contains(&"modern::value"));
    }

    #[test]
    fn legacy_module_blocks_parse_multiple_modules_in_one_file() {
        let package = move_package(
            r#"
module demo::first {
    fun a() { if (true) {} }
}

module demo::second {
    public fun b() { while (false) {} }
}
"#,
        );
        let report = Analyzer::new().analyze_package(package.path(), AnalysisConfig::default());
        let targets = report
            .metrics
            .iter()
            .filter(|metric| metric.rule_id == FUNCTION_RULE_ID)
            .map(|metric| metric.target.as_str())
            .collect::<Vec<_>>();

        assert!(targets.contains(&"first::a"));
        assert!(targets.contains(&"second::b"));
    }

    #[test]
    fn comments_strings_and_escaped_keywords_do_not_add_complexity() {
        let package = move_package(
            r#"
module demo::m;

fun simple() {
    // if while loop match assert! abort && || transfer::share_object(object::new(ctx))
    /* if (true) { abort 1 } */
    let _bytes = b"if match assert! abort && ||";
    let r#match = 1;
    r#match
}
"#,
        );
        let report = Analyzer::new().analyze_package(package.path(), AnalysisConfig::default());
        let metric = report
            .metrics
            .iter()
            .find(|metric| metric.rule_id == FUNCTION_RULE_ID && metric.target == "m::simple")
            .expect("function metric");

        assert_eq!(metric.metric.value, 1);
    }

    #[test]
    fn function_threshold_emits_finding() {
        let package = move_package(
            r#"
module demo::m {
    fun complicated() {
        if (true) {};
        if (true) {};
    }
}
"#,
        );
        let mut config = AnalysisConfig::default();
        config
            .analysis
            .rulesets
            .get_mut(RULESET_ID)
            .unwrap()
            .rules
            .get_mut(FUNCTION_RULE_ID)
            .unwrap()
            .threshold = Some(2);

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule_id == FUNCTION_RULE_ID));
    }

    #[test]
    fn module_threshold_emits_finding() {
        let package = move_package(
            r#"
module demo::m {
    fun a() { if (true) {} }
    fun b() { if (true) {} }
}
"#,
        );
        let mut config = AnalysisConfig::default();
        config
            .analysis
            .rulesets
            .get_mut(RULESET_ID)
            .unwrap()
            .rules
            .get_mut(MODULE_RULE_ID)
            .unwrap()
            .threshold = Some(3);

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule_id == MODULE_RULE_ID));
    }

    #[test]
    fn inactive_rules_are_honored() {
        let package = move_package(
            r#"
module demo::m {
    fun complicated() {
        if (true) {};
        if (true) {};
    }
}
"#,
        );
        let mut config = AnalysisConfig::default();
        config
            .analysis
            .rulesets
            .get_mut(RULESET_ID)
            .unwrap()
            .rules
            .get_mut(FUNCTION_RULE_ID)
            .unwrap()
            .active = Some(false);

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(!report
            .metrics
            .iter()
            .any(|metric| metric.rule_id == FUNCTION_RULE_ID));
    }

    #[test]
    fn custom_thresholds_from_config_are_honored() {
        let package = move_package(
            r#"
module demo::m {
    public entry fun almost_simple() { if (true) {} }
}
"#,
        );
        let config = toml::from_str::<AnalysisConfig>(
            r#"
[analysis.rulesets.complexity]
active = true

[analysis.rulesets.complexity.FunctionComplexity]
active = true
threshold = 100
entry_threshold = 2
"#,
        )
        .expect("config should parse");

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule_id == FUNCTION_RULE_ID));
    }

    fn move_package(source: &str) -> TempDir {
        let temp = tempfile::tempdir().expect("temp package");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.0.1\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources dir");
        fs::write(temp.path().join("sources/m.move"), source).expect("source");
        temp
    }

    #[test]
    fn default_config_contains_complexity_rules() {
        let config = AnalysisConfig::default();
        let ruleset = config.analysis.rulesets.get(RULESET_ID).unwrap();

        assert!(ruleset.is_active());
        assert!(ruleset.rule_config(FUNCTION_RULE_ID).is_active());
        assert!(ruleset.rule_config(MODULE_RULE_ID).is_active());
    }

    #[test]
    fn ruleset_config_can_disable_everything() {
        let package = move_package(
            r#"
module demo::m {
    fun complicated() {
        if (true) {};
    }
}
"#,
        );
        let mut rulesets = BTreeMap::new();
        rulesets.insert(
            RULESET_ID.to_string(),
            RuleSetConfig {
                active: Some(false),
                rules: BTreeMap::new(),
            },
        );
        let mut config = AnalysisConfig::default();
        config.analysis.rulesets = rulesets;

        let report = Analyzer::new().analyze_package(package.path(), config);

        assert!(report.loaded_rulesets.is_empty());
        assert!(report.metrics.is_empty());
    }
}
