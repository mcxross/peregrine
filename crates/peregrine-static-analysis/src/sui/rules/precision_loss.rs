use peregrine_types::analysis::{
    AnalysisContext, Rule, RuleConfig, RuleMetadata, RuleOutcome, Severity,
};

use super::common::{
    all_functions, finding, function_target, rule_metadata, sanitize_source, token_line_span,
    token_range_contains, token_range_contains_call, tokenize, Token,
};

pub const RULE_ID: &str = "precision_loss";

pub struct PrecisionLossRule;

impl Rule for PrecisionLossRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn metadata(&self) -> RuleMetadata {
        rule_metadata(
            RULE_ID,
            "Precision loss",
            "Reports multiplication of values already reduced by division or square root.",
            Severity::Warning,
        )
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();

        for (module, function) in all_functions(context) {
            let sanitized = sanitize_source(&function.body);
            let tokens = tokenize(&sanitized);

            for (index, token) in tokens.iter().enumerate() {
                if token.text != "*" {
                    continue;
                }
                let Some((left_start, left_end)) = left_operand_range(&tokens, index) else {
                    continue;
                };
                let Some((right_start, right_end)) = right_operand_range(&tokens, index) else {
                    continue;
                };
                let risky_left = operand_loses_precision(&tokens, left_start, left_end);
                let risky_right = operand_loses_precision(&tokens, right_start, right_end);

                if !risky_left && !risky_right {
                    continue;
                }

                let target = function_target(module, function);
                outcome.findings.push(finding(
                    RULE_ID,
                    RULE_ID,
                    Severity::Warning,
                    format!("{target} multiplies a value derived from division or `sqrt`; multiply before reducing precision."),
                    function.file.clone(),
                    token_line_span(&sanitized, token, function.span),
                ));
            }
        }

        outcome
    }
}

fn operand_loses_precision(tokens: &[Token], start: usize, end: usize) -> bool {
    token_range_contains(tokens, start, end, "/")
        || token_range_contains_call(tokens, start, end, "sqrt")
}

fn left_operand_range(tokens: &[Token], operator_index: usize) -> Option<(usize, usize)> {
    if operator_index == 0 {
        return None;
    }
    let mut depth = 0_i32;
    let mut cursor = operator_index - 1;

    loop {
        let token = &tokens[cursor];
        match token.text.as_str() {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                depth -= 1;
                if depth < 0 {
                    return Some((cursor + 1, operator_index - 1));
                }
            }
            _ if depth == 0 && is_operand_boundary(token.text.as_str()) => {
                return Some((cursor + 1, operator_index - 1));
            }
            _ => {}
        }

        if cursor == 0 {
            return Some((0, operator_index - 1));
        }
        cursor -= 1;
    }
}

fn right_operand_range(tokens: &[Token], operator_index: usize) -> Option<(usize, usize)> {
    let mut depth = 0_i32;
    let mut cursor = operator_index + 1;

    while cursor < tokens.len() {
        let token = &tokens[cursor];
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    return Some((operator_index + 1, cursor.saturating_sub(1)));
                }
                depth -= 1;
            }
            _ if depth == 0 && is_operand_boundary(token.text.as_str()) => {
                return Some((operator_index + 1, cursor.saturating_sub(1)));
            }
            _ => {}
        }
        cursor += 1;
    }

    Some((operator_index + 1, tokens.len().saturating_sub(1)))
}

fn is_operand_boundary(text: &str) -> bool {
    matches!(
        text,
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "=="
            | "!="
            | "<"
            | ">"
            | "<="
            | ">="
            | "&&"
            | "||"
            | "="
            | ","
            | ";"
            | "=>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::analysis::{
        AnalysisConfig, AnalysisContext, ParsedFunction, ParsedModule, SourceFile, Span,
    };
    use std::path::PathBuf;

    #[test]
    fn flags_multiplication_after_division() {
        let outcome = PrecisionLossRule.analyze(
            &context("fun f(a: u64, b: u64, c: u64): u64 { (a / b) * c }"),
            &RuleConfig::default(),
        );

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn does_not_flag_multiply_before_division() {
        let outcome = PrecisionLossRule.analyze(
            &context("fun f(a: u64, b: u64, c: u64): u64 { (a * b) / c }"),
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
                    signature: "fun f(a: u64, b: u64, c: u64): u64".to_string(),
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
