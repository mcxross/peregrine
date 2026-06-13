use peregrine_types::analysis::{
    AnalysisContext, Rule, RuleConfig, RuleMetadata, RuleOutcome, Severity,
};

use super::common::{
    all_functions, find_matching_token, finding, function_target, rule_metadata, sanitize_source,
    token_line_span, tokenize,
};

pub const RULE_ID: &str = "infinite_loop";

pub struct InfiniteLoopRule;

impl Rule for InfiniteLoopRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn metadata(&self) -> RuleMetadata {
        rule_metadata(
            RULE_ID,
            "Infinite loop",
            "Reports loops with no obvious break, return, or abort path.",
            Severity::Warning,
        )
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();

        for (module, function) in all_functions(context) {
            let sanitized = sanitize_source(&function.body);
            let tokens = tokenize(&sanitized);

            for (index, token) in tokens.iter().enumerate() {
                let loop_body_start = match token.text.as_str() {
                    "loop" => find_next_token(&tokens, index + 1, "{"),
                    "while" if has_constant_true_condition(&tokens, index) => {
                        find_next_token(&tokens, index + 1, "{")
                    }
                    _ => None,
                };
                let Some(body_start) = loop_body_start else {
                    continue;
                };
                let Some(body_end) = find_matching_token(&tokens, body_start, "{", "}") else {
                    continue;
                };

                if loop_body_can_exit(&tokens[body_start + 1..body_end]) {
                    continue;
                }

                let target = function_target(module, function);
                outcome.findings.push(finding(
                    RULE_ID,
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "{target} contains a loop with no obvious break, return, or abort path."
                    ),
                    function.file.clone(),
                    token_line_span(&sanitized, token, function.span),
                ));
            }
        }

        outcome
    }
}

fn has_constant_true_condition(tokens: &[super::common::Token], while_index: usize) -> bool {
    let Some(open_index) = find_next_token(tokens, while_index + 1, "(") else {
        return false;
    };
    let Some(close_index) = find_matching_token(tokens, open_index, "(", ")") else {
        return false;
    };
    close_index == open_index + 2
        && tokens
            .get(open_index + 1)
            .is_some_and(|token| token.text == "true")
}

fn find_next_token(tokens: &[super::common::Token], start: usize, text: &str) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, token)| (token.text == text).then_some(index))
}

fn loop_body_can_exit(tokens: &[super::common::Token]) -> bool {
    tokens
        .iter()
        .any(|token| matches!(token.text.as_str(), "break" | "return" | "abort"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::analysis::{
        AnalysisConfig, AnalysisContext, ParsedFunction, ParsedModule, SourceFile, Span,
    };
    use std::path::PathBuf;

    #[test]
    fn flags_loop_without_exit() {
        let outcome = InfiniteLoopRule.analyze(
            &context("fun f() { loop { let x = 1; } }"),
            &RuleConfig::default(),
        );

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn allows_loop_with_break() {
        let outcome = InfiniteLoopRule.analyze(
            &context("fun f(flag: bool) { loop { if (flag) { break } } }"),
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
                    signature: "fun f()".to_string(),
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
