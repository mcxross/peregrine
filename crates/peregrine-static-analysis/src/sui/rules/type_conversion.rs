use peregrine_types::analysis::{
    AnalysisContext, Rule, RuleConfig, RuleMetadata, RuleOutcome, Severity,
};

use super::common::{
    all_functions, finding, function_local_types, function_target, primitive_type_after_cast,
    rule_metadata, sanitize_source, token_is_identifier, token_line_span, tokenize, Token,
};

pub const RULE_ID: &str = "type_conversion";

pub struct TypeConversionRule;

impl Rule for TypeConversionRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn metadata(&self) -> RuleMetadata {
        rule_metadata(
            RULE_ID,
            "Type conversion",
            "Reports no-op primitive casts.",
            Severity::Info,
        )
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();

        for (module, function) in all_functions(context) {
            let local_types = function_local_types(function);
            let sanitized = sanitize_source(&function.body);
            let tokens = tokenize(&sanitized);

            for (index, token) in tokens.iter().enumerate() {
                if token.text != "as" {
                    continue;
                }
                let Some(target_type) = primitive_type_after_cast(&tokens, index) else {
                    continue;
                };
                let Some(source_name) = cast_source_name(&tokens, index) else {
                    continue;
                };
                if local_types.get(source_name) != Some(&target_type.to_string()) {
                    continue;
                }

                let target = function_target(module, function);
                outcome.findings.push(finding(
                    RULE_ID,
                    RULE_ID,
                    Severity::Info,
                    format!("{target} casts `{source_name}` to `{target_type}` even though it is already `{target_type}`."),
                    function.file.clone(),
                    token_line_span(&sanitized, token, function.span),
                ));
            }
        }

        outcome
    }
}

fn cast_source_name<'a>(tokens: &'a [Token], cast_index: usize) -> Option<&'a str> {
    let previous = cast_index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))?;
    if token_is_identifier(&previous.text) {
        return Some(previous.text.as_str());
    }

    if previous.text == ")" {
        let mut depth = 0_i32;
        let mut cursor = cast_index.checked_sub(1)?;
        loop {
            let token = &tokens[cursor];
            match token.text.as_str() {
                ")" => depth += 1,
                "(" => {
                    depth -= 1;
                    if depth == 0 {
                        return tokens
                            .get(cursor + 1)
                            .filter(|candidate| token_is_identifier(&candidate.text))
                            .map(|candidate| candidate.text.as_str());
                    }
                }
                _ => {}
            }
            if cursor == 0 {
                break;
            }
            cursor -= 1;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::analysis::{
        AnalysisConfig, AnalysisContext, ParsedFunction, ParsedModule, SourceFile, Span,
    };
    use std::path::PathBuf;

    #[test]
    fn flags_noop_cast_from_parameter() {
        let outcome = TypeConversionRule.analyze(
            &context("fun f(amount: u64): u64 { amount as u64 }"),
            &RuleConfig::default(),
        );

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn ignores_real_widening_cast() {
        let outcome = TypeConversionRule.analyze(
            &context("fun f(amount: u64): u128 { amount as u128 }"),
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
                    signature: "fun f(amount: u64): u64".to_string(),
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
