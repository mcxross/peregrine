use peregrine_types::analysis::{
    AnalysisContext, Rule, RuleConfig, RuleMetadata, RuleOutcome, Severity,
};

use super::common::{
    all_functions, finding, function_target, rule_metadata, sanitize_source, token_line_span,
    tokenize,
};

pub const RULE_ID: &str = "bool_judgement";

pub struct BoolJudgementRule;

impl Rule for BoolJudgementRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn metadata(&self) -> RuleMetadata {
        rule_metadata(
            RULE_ID,
            "Boolean judgement",
            "Reports boolean literals compared with boolean expressions.",
            Severity::Info,
        )
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();

        for (module, function) in all_functions(context) {
            let sanitized = sanitize_source(&function.body);
            let tokens = tokenize(&sanitized);

            for (index, token) in tokens.iter().enumerate() {
                if token.text != "true" && token.text != "false" {
                    continue;
                }

                let compared_left = index
                    .checked_sub(1)
                    .and_then(|index| tokens.get(index))
                    .is_some_and(|candidate| candidate.text == "==" || candidate.text == "!=");
                let compared_right = tokens
                    .get(index + 1)
                    .is_some_and(|candidate| candidate.text == "==" || candidate.text == "!=");

                if !compared_left && !compared_right {
                    continue;
                }

                let target = function_target(module, function);
                outcome.findings.push(finding(
                    RULE_ID,
                    RULE_ID,
                    Severity::Info,
                    format!(
                        "{target} compares a boolean expression with `{}`; use the boolean expression directly.",
                        token.text
                    ),
                    function.file.clone(),
                    token_line_span(&sanitized, token, function.span),
                ));
            }
        }

        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::analysis::{
        AnalysisConfig, AnalysisContext, ParsedFunction, ParsedModule, SourceFile, Span,
    };
    use std::path::PathBuf;

    #[test]
    fn flags_bool_literal_comparisons() {
        let outcome = BoolJudgementRule.analyze(
            &context("fun f(flag: bool) { if (flag == true) {} }"),
            &RuleConfig::default(),
        );

        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].ruleset_id, RULE_ID);
        assert_eq!(outcome.findings[0].rule_id, RULE_ID);
    }

    #[test]
    fn ignores_bool_literals_not_used_in_comparisons() {
        let outcome = BoolJudgementRule.analyze(
            &context("fun f(flag: bool) { if (flag) { let x = true; } }"),
            &RuleConfig::default(),
        );

        assert!(outcome.findings.is_empty());
    }

    fn context(body: &str) -> AnalysisContext {
        AnalysisContext {
            package_path: PathBuf::from("/workspace"),
            source_files: vec![SourceFile {
                path: "sources/m.move".to_string(),
                contents: body.to_string(),
            }],
            modules: vec![ParsedModule {
                name: "m".to_string(),
                address: Some("demo".to_string()),
                file: "sources/m.move".to_string(),
                functions: vec![ParsedFunction {
                    module_name: "m".to_string(),
                    name: "f".to_string(),
                    visibility: "private".to_string(),
                    is_entry: false,
                    is_transaction_callable: false,
                    signature: "fun f(flag: bool)".to_string(),
                    body: body.to_string(),
                    file: "sources/m.move".to_string(),
                    span: Some(Span {
                        start_line: 1,
                        end_line: body.lines().count().max(1),
                    }),
                    type_parameter_count: 0,
                }],
            }],
            config: AnalysisConfig::default(),
        }
    }
}
