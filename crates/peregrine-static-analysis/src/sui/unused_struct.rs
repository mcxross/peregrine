use peregrine_analysis_core::{AnalysisContext, Rule, RuleConfig, RuleOutcome, Severity};

use super::common::{
    collect_declarations, finding, name_referenced_outside_declaration, DeclaredItemKind,
};

pub const RULE_ID: &str = "UnusedStruct";

pub struct UnusedStructRule;

impl Rule for UnusedStructRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn analyze(&self, context: &AnalysisContext, _config: &RuleConfig) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();

        for item in collect_declarations(context).into_iter().filter(|item| {
            matches!(item.kind, DeclaredItemKind::Struct | DeclaredItemKind::Enum)
                && !item.is_test_only
        }) {
            if name_referenced_outside_declaration(context, &item) {
                continue;
            }

            let kind = match item.kind {
                DeclaredItemKind::Struct => "Struct",
                DeclaredItemKind::Enum => "Enum",
                DeclaredItemKind::Const => "Constant",
            };
            outcome.findings.push(finding(
                RULE_ID,
                Severity::Info,
                format!("{kind} `{}` is defined but never referenced.", item.name),
                item.file,
                Some(item.span),
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
    fn flags_unused_struct() {
        let outcome = UnusedStructRule.analyze(
            &context("module demo::m; public struct Vault has key { id: UID } public fun f() {}"),
            &RuleConfig::default(),
        );

        assert_eq!(outcome.findings.len(), 1);
    }

    #[test]
    fn ignores_referenced_struct() {
        let outcome = UnusedStructRule.analyze(
            &context(
                "module demo::m; public struct Vault has key { id: UID } public fun f(v: &Vault) {}",
            ),
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
