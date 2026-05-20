use std::path::Path;

use crate::engine::{AnalysisEngine, AnalysisEngineOptions};
use peregrine_types::analysis::{AnalysisConfig, AnalysisReport, RuleSetProvider};

pub struct Analyzer {
    engine: AnalysisEngine,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::new(),
        }
    }

    pub fn with_provider(mut self, provider: Box<dyn RuleSetProvider>) -> Self {
        self.engine = self.engine.with_provider(provider);
        self
    }

    pub fn analyze_package(
        &self,
        package_path: impl AsRef<Path>,
        config: AnalysisConfig,
    ) -> AnalysisReport {
        self.engine.analyze_package_with_options(
            package_path,
            config,
            AnalysisEngineOptions::without_global_plugins(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn default_analyzer_loads_sui_ruleset_for_move_package() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            r#"
[package]
name = "demo"
"#,
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("sources/m.move"),
            r#"
module demo::m;

public fun flags(flag: bool) {
    if (flag == true) {};
}
"#,
        )
        .expect("source");

        let report = Analyzer::new().analyze_package(temp.path(), AnalysisConfig::default());

        assert!(report
            .loaded_rulesets
            .iter()
            .any(|ruleset| ruleset == "bool_judgement"));
        assert!(report.findings.iter().any(|finding| {
            finding.ruleset_id == "bool_judgement" && finding.rule_id == "bool_judgement"
        }));
    }
}
