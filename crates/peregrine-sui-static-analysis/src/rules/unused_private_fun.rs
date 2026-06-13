use peregrine_types::analysis::{
    AnalysisContext, ParsedFunction, ParsedModule, Rule, RuleConfig, RuleMetadata, RuleOutcome,
    Severity,
};

use super::common::{
    all_functions, called_by_function, finding, function_target, rule_metadata, test_like_function,
};

pub const RULE_ID: &str = "unused_private_function";

pub struct UnusedPrivateFunctionRule;

impl Rule for UnusedPrivateFunctionRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn metadata(&self) -> RuleMetadata {
        rule_metadata(
            RULE_ID,
            "Unused private function",
            "Reports private package functions that are not invoked.",
            Severity::Info,
        )
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let candidates = all_functions(context)
            .into_iter()
            .filter(|(_, function)| is_unused_candidate(function))
            .collect::<Vec<_>>();
        let callers = all_functions(context);
        let mut outcome = RuleOutcome::default();

        for (module, function) in candidates {
            let used = callers.iter().any(|(caller_module, caller)| {
                if caller_module.name == module.name && caller.name == function.name {
                    return false;
                }
                called_by_function(caller_module, caller, &module.name, &function.name)
            });

            if used {
                continue;
            }

            outcome.findings.push(finding(
                RULE_ID,
                RULE_ID,
                Severity::Info,
                format!(
                    "{} is not invoked by any package function.",
                    function_target(module, function)
                ),
                function.file.clone(),
                function.span,
            ));
        }

        outcome
    }
}

fn is_unused_candidate(function: &ParsedFunction) -> bool {
    matches!(function.visibility.as_str(), "private" | "public(friend)")
        && !function.is_entry
        && function.name != "init"
        && !test_like_function(function)
}

#[allow(dead_code)]
fn _candidate_label(module: &ParsedModule, function: &ParsedFunction) -> String {
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
    fn flags_unused_private_function() {
        let context = context(vec![
            function("helper", "private", "fun helper() {}"),
            function("entry", "public", "public fun entry() {}"),
        ]);

        let outcome = UnusedPrivateFunctionRule.analyze(&context, &RuleConfig::default());

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn ignores_called_private_function() {
        let context = context(vec![
            function("helper", "private", "fun helper() {}"),
            function("entry", "public", "public fun entry() { helper(); }"),
        ]);

        let outcome = UnusedPrivateFunctionRule.analyze(&context, &RuleConfig::default());

        assert!(outcome.findings.is_empty());
    }

    fn function(name: &str, visibility: &str, body: &str) -> ParsedFunction {
        ParsedFunction {
            module_name: "m".to_string(),
            name: name.to_string(),
            visibility: visibility.to_string(),
            is_entry: false,
            is_transaction_callable: visibility == "public",
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
