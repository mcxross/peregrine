use std::collections::BTreeMap;

use peregrine_types::analysis::{
    AnalysisContext, Finding, Metric, ParsedFunction, ParsedModule, Rule, RuleConfig, RuleMetric,
    RuleOutcome, RuleSet, RuleSetProvider, Severity,
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
    use peregrine_types::analysis::{AnalysisConfig, RuleConfig, SourceFile, Span};
    use std::path::PathBuf;

    #[test]
    fn simple_function_has_base_complexity() {
        assert_eq!(
            function_complexity_score(&test_function("id", "fun id() { 1 }")),
            1
        );
    }

    #[test]
    fn counts_move_specific_complexity() {
        let function = ParsedFunction {
            module_name: "m".to_string(),
            name: "mutate".to_string(),
            visibility: "public".to_string(),
            is_entry: true,
            is_transaction_callable: true,
            signature: "public entry fun mutate<T>(flag: bool, ctx: &mut TxContext)".to_string(),
            body: r#"
public entry fun mutate<T>(flag: bool, ctx: &mut TxContext) {
    if (flag && true) {
        assert!(flag, 0);
        transfer::share_object(object::new(ctx));
    } else if (!flag || false) {
        loop { abort 1 }
    };
}
"#
            .to_string(),
            file: "sources/m.move".to_string(),
            span: Some(Span {
                start_line: 1,
                end_line: 9,
            }),
            type_parameter_count: 1,
        };

        assert_eq!(function_complexity_score(&function), 14);
    }

    #[test]
    fn comments_strings_and_escaped_keywords_do_not_add_complexity() {
        let function = test_function(
            "simple",
            r#"
fun simple() {
    // if while loop match assert! abort && || transfer::share_object(object::new(ctx))
    /* if (true) { abort 1 } */
    let _bytes = b"if match assert! abort && ||";
    let r#match = 1;
    r#match
}
"#,
        );

        assert_eq!(function_complexity_score(&function), 1);
    }

    #[test]
    fn function_threshold_emits_finding() {
        let context = context(vec![ParsedModule {
            name: "m".to_string(),
            address: Some("demo".to_string()),
            file: "sources/m.move".to_string(),
            functions: vec![test_function(
                "complicated",
                r#"
fun complicated() {
        if (true) {};
        if (true) {};
}
"#,
            )],
        }]);
        let config = RuleConfig {
            threshold: Some(2),
            ..RuleConfig::default()
        };

        let outcome = FunctionComplexityRule.analyze(&context, &config);

        assert!(outcome
            .findings
            .iter()
            .any(|finding| finding.rule_id == FUNCTION_RULE_ID));
    }

    #[test]
    fn module_threshold_emits_finding() {
        let context = context(vec![ParsedModule {
            name: "m".to_string(),
            address: Some("demo".to_string()),
            file: "sources/m.move".to_string(),
            functions: vec![
                test_function("a", "fun a() { if (true) {} }"),
                test_function("b", "fun b() { if (true) {} }"),
            ],
        }]);
        let config = RuleConfig {
            threshold: Some(3),
            ..RuleConfig::default()
        };

        let outcome = ModuleComplexityRule.analyze(&context, &config);

        assert!(outcome
            .findings
            .iter()
            .any(|finding| finding.rule_id == MODULE_RULE_ID));
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
    fn custom_entry_threshold_from_config_is_honored() {
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
        let ruleset = config.analysis.rulesets.get(RULESET_ID).unwrap();
        let rule_config = ruleset.rule_config(FUNCTION_RULE_ID);
        let context = context(vec![ParsedModule {
            name: "m".to_string(),
            address: Some("demo".to_string()),
            file: "sources/m.move".to_string(),
            functions: vec![ParsedFunction {
                is_entry: true,
                is_transaction_callable: true,
                visibility: "public".to_string(),
                signature: "public entry fun almost_simple()".to_string(),
                type_parameter_count: 0,
                ..test_function(
                    "almost_simple",
                    "public entry fun almost_simple() { if (true) {} }",
                )
            }],
        }]);

        let outcome = FunctionComplexityRule.analyze(&context, &rule_config);

        assert!(outcome
            .findings
            .iter()
            .any(|finding| finding.rule_id == FUNCTION_RULE_ID));
    }

    fn test_function(name: &str, body: &str) -> ParsedFunction {
        ParsedFunction {
            module_name: "m".to_string(),
            name: name.to_string(),
            visibility: "private".to_string(),
            is_entry: false,
            is_transaction_callable: false,
            signature: format!("fun {name}()"),
            body: body.to_string(),
            file: "sources/m.move".to_string(),
            span: Some(Span {
                start_line: 1,
                end_line: body.lines().count().max(1),
            }),
            type_parameter_count: 0,
        }
    }

    fn context(modules: Vec<ParsedModule>) -> AnalysisContext {
        AnalysisContext {
            package_path: PathBuf::from("/workspace"),
            source_files: vec![SourceFile {
                path: "sources/m.move".to_string(),
                contents: String::new(),
            }],
            modules,
            config: AnalysisConfig::default(),
        }
    }
}
