mod analyzer;
mod config;
mod model;
mod parser;
mod plugins;
pub mod rules;

pub use analyzer::{Analyzer, Rule, RuleSet, RuleSetProvider};
pub use config::{AnalysisConfig, AnalysisSection, PluginConfig, RuleConfig, RuleSetConfig};
pub use model::{
    AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, Metric, ParsedFunction,
    ParsedModule, RuleMetric, Severity, SourceFile, Span,
};
pub use plugins::{
    PluginAnalyzeInput, PluginAnalyzeOutput, PluginManifest, PluginManifestInput,
    PluginRuleManifest, PluginRuleSetManifest, WasmPluginHost,
};
