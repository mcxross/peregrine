mod analysis;
mod analyzer;
mod engine;
mod parser;
mod plugins;
mod project;
pub mod rules;

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

pub use analysis::SuiStaticAnalyzer;
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
    MovePackage, MoveProject, discover_move_project, discover_move_project_fast,
    discover_move_project_shallow,
};
pub use rules::SuiRuleSetProvider;
