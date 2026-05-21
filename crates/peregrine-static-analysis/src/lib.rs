mod analyzer;
mod engine;
mod parser;
mod plugins;
mod project;
pub mod sui;

pub mod config {
    pub use peregrine_types::analysis::{
        AnalysisConfig, AnalysisSection, PluginConfig, RuleConfig, RuleSetConfig,
    };
}

pub mod model {
    pub use peregrine_types::analysis::{
        AnalysisContext, AnalysisDiagnostic, AnalysisReport, AnalysisRuleCatalog, Finding, Metric,
        ParsedFunction, ParsedModule, RuleConfigProperty, RuleConfigValueKind, RuleMetadata,
        RuleMetric, RuleSetMetadata, Severity, SourceFile, Span,
    };
}

pub mod rules {
    pub mod complexity {
        pub use crate::sui::rules::complexity::*;
    }

    pub mod sui {
        pub use crate::sui::rules::*;
    }
}

pub use analyzer::Analyzer;
pub use engine::{AnalysisEngine, AnalysisEngineOptions};
pub use peregrine_plugins::{
    InstalledPlugin, PluginInstallManifest, PluginKind, PluginRegistry, PluginRegistryFile,
    PluginRuntimeKind,
};
pub use peregrine_types::analysis::{
    AnalysisConfig, AnalysisSection, PluginConfig, RuleConfig, RuleSetConfig,
};
pub use peregrine_types::analysis::{
    AnalysisContext, AnalysisDiagnostic, AnalysisReport, AnalysisRuleCatalog, Finding, Metric,
    ParsedFunction, ParsedModule, Rule, RuleConfigProperty, RuleConfigValueKind, RuleMetadata,
    RuleMetric, RuleOutcome, RuleSet, RuleSetMetadata, RuleSetProvider, Severity, SourceFile, Span,
};
pub use plugins::{
    AnalysisPluginHost, AnalyzerPluginRegistry, AnalyzerPluginRegistryFile,
    InstalledAnalyzerPlugin, PluginActiveRuleConfig, PluginAnalysisReport, PluginAnalyzeInput,
    PluginAnalyzeOutput, PluginManifest, PluginManifestInput, PluginRuleManifest,
    PluginRuleSetManifest,
};
pub use project::{
    discover_move_project, discover_move_project_fast, discover_move_project_shallow, MovePackage,
    MoveProject,
};
