use std::collections::BTreeSet;

use peregrine_types::analysis::{
    AnalysisContext, ParsedFunction, ParsedModule, Rule, RuleConfig, RuleMetadata, RuleOutcome,
    Severity,
};

use super::common::{
    Token, all_functions, call_name_at, find_matching_token, finding, function_has_return_value,
    function_target, is_function_declaration, qualified_call_module, rule_metadata,
    sanitize_source, token_line_span, tokenize,
};

pub const RULE_ID: &str = "unchecked_return";

pub struct UncheckedReturnRule;

impl Rule for UncheckedReturnRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn metadata(&self) -> RuleMetadata {
        rule_metadata(
            RULE_ID,
            "Unchecked return",
            "Reports calls whose return values are discarded.",
            Severity::Info,
        )
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let returning_functions = returning_function_set(context);
        let mut outcome = RuleOutcome::default();

        for (module, function) in all_functions(context) {
            let sanitized = sanitize_source(&function.body);
            let tokens = tokenize(&sanitized);

            for (index, token) in tokens.iter().enumerate() {
                if is_function_declaration(&tokens, index) {
                    continue;
                }
                let Some((callee_name, open_index)) = call_name_at(&tokens, index) else {
                    continue;
                };
                let callee_module = qualified_call_module(&tokens, index).unwrap_or(&module.name);
                if !returning_functions
                    .contains(&(callee_module.to_string(), callee_name.to_string()))
                {
                    continue;
                }
                if !return_is_unchecked(&tokens, index, open_index) {
                    continue;
                }

                let target = function_target(module, function);
                outcome.findings.push(finding(
                    RULE_ID,
                    RULE_ID,
                    Severity::Info,
                    format!(
                        "{target} discards the return value from `{callee_module}::{callee_name}`."
                    ),
                    function.file.clone(),
                    token_line_span(&sanitized, token, function.span),
                ));
            }
        }

        outcome
    }
}

fn returning_function_set(context: &AnalysisContext) -> BTreeSet<(String, String)> {
    all_functions(context)
        .into_iter()
        .filter(|(_, function)| function_has_return_value(function))
        .map(|(module, function)| (module.name.clone(), function.name.clone()))
        .collect()
}

fn return_is_unchecked(tokens: &[Token], call_index: usize, open_index: usize) -> bool {
    let Some(close_index) = find_matching_token(tokens, open_index, "(", ")") else {
        return false;
    };
    let previous = call_index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.text.as_str());
    let next = tokens.get(close_index + 1).map(|token| token.text.as_str());

    if previous == Some("return") {
        return false;
    }

    if previous == Some("=") {
        return assignment_discards_result(tokens, call_index);
    }

    next == Some(";")
}

fn assignment_discards_result(tokens: &[Token], call_index: usize) -> bool {
    let Some(equal_index) = call_index.checked_sub(1) else {
        return false;
    };
    let Some(lhs) = equal_index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
    else {
        return false;
    };

    lhs.text == "_" || lhs.text.starts_with('_')
}

#[allow(dead_code)]
fn _label(module: &ParsedModule, function: &ParsedFunction) -> String {
    format!("{}::{}", module.name, function.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::analysis::{
        AnalysisConfig, AnalysisContext, ParsedFunction, ParsedModule, SourceFile, Span,
    };
    use std::path::PathBuf;

    #[test]
    fn flags_ignored_local_return() {
        let context = context(vec![
            function("value", "fun value(): u64 { 1 }", "fun value(): u64"),
            function("caller", "fun caller() { value(); }", "fun caller()"),
        ]);

        let outcome = UncheckedReturnRule.analyze(&context, &RuleConfig::default());

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn allows_assigned_return() {
        let context = context(vec![
            function("value", "fun value(): u64 { 1 }", "fun value(): u64"),
            function(
                "caller",
                "fun caller() { let x = value(); x; }",
                "fun caller()",
            ),
        ]);

        let outcome = UncheckedReturnRule.analyze(&context, &RuleConfig::default());

        assert!(outcome.findings.is_empty());
    }

    fn function(name: &str, body: &str, signature: &str) -> ParsedFunction {
        ParsedFunction {
            module_name: "m".to_string(),
            name: name.to_string(),
            visibility: "private".to_string(),
            is_entry: false,
            is_transaction_callable: false,
            signature: signature.to_string(),
            body: body.to_string(),
            file: "sources/m.move".to_string(),
            span: Some(Span {
                start_line: 1,
                end_line: body.lines().count().max(1),
            }),
            type_parameter_count: 0,
        }
    }

    fn context(functions: Vec<ParsedFunction>) -> AnalysisContext {
        AnalysisContext {
            package_path: PathBuf::from("/workspace"),
            source_files: vec![SourceFile {
                path: "sources/m.move".to_string(),
                contents: functions
                    .iter()
                    .map(|function| function.body.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            }],
            modules: vec![ParsedModule {
                name: "m".to_string(),
                address: Some("demo".to_string()),
                file: "sources/m.move".to_string(),
                functions,
            }],
            config: AnalysisConfig::default(),
        }
    }
}
