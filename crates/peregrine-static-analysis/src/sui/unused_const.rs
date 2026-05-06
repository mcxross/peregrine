use peregrine_analysis_core::{AnalysisContext, Rule, RuleConfig, RuleOutcome, Severity};

use super::common::{
    collect_declarations, finding, name_referenced_outside_declaration, DeclaredItemKind,
};

pub const RULE_ID: &str = "UnusedConst";

pub struct UnusedConstRule;

impl Rule for UnusedConstRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();

        for constant in collect_declarations(context)
            .into_iter()
            .filter(|item| item.kind == DeclaredItemKind::Const && !item.is_test_only)
        {
            if name_referenced_outside_declaration(context, &constant) {
                continue;
            }

            outcome.findings.push(finding(
                RULE_ID,
                Severity::Info,
                format!(
                    "Constant `{}` is defined but never referenced.",
                    constant.name
                ),
                constant.file,
                Some(constant.span),
            ));
        }

        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_analysis_core::{AnalysisConfig, AnalysisContext, ParsedModule, SourceFile};
    use std::path::PathBuf;

    #[test]
    fn flags_unused_constant() {
        let outcome = UnusedConstRule.analyze(
            &context("module demo::m; const FEE: u64 = 1; public fun f() {}"),
            &RuleConfig::default(),
        );

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn ignores_referenced_constant() {
        let outcome = UnusedConstRule.analyze(
            &context("module demo::m; const FEE: u64 = 1; public fun f(): u64 { FEE }"),
            &RuleConfig::default(),
        );

        assert!(outcome.findings.is_empty());
    }

    fn context(source: &str) -> AnalysisContext {
        AnalysisContext {
            package_path: PathBuf::from("/workspace"),
            source_files: vec![SourceFile {
                path: "sources/m.move".to_string(),
                contents: source.to_string(),
            }],
            modules: vec![ParsedModule {
                name: "m".to_string(),
                address: Some("demo".to_string()),
                file: "sources/m.move".to_string(),
                functions: Vec::new(),
            }],
            config: AnalysisConfig::default(),
        }
    }
}
