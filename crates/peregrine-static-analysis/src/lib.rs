mod analyzer;
mod parser;
mod plugins;
mod project;

pub mod config {
    pub use peregrine_analysis_core::{
        AnalysisConfig, AnalysisSection, PluginConfig, RuleConfig, RuleSetConfig,
    };
}

pub mod model {
    pub use peregrine_analysis_core::{
        AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, Metric, ParsedFunction,
        ParsedModule, RuleMetric, Severity, SourceFile, Span,
    };
}

pub mod rules {
    pub mod complexity {
        pub use peregrine_complexity_rules::*;
    }
}

pub use analyzer::Analyzer;
pub use peregrine_analysis_core::{
    AnalysisConfig, AnalysisSection, PluginConfig, RuleConfig, RuleSetConfig,
};
pub use peregrine_analysis_core::{
    AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, Metric, ParsedFunction,
    ParsedModule, Rule, RuleMetric, RuleOutcome, RuleSet, RuleSetProvider, Severity, SourceFile,
    Span,
};
pub use plugins::{
    PluginAnalyzeInput, PluginAnalyzeOutput, PluginManifest, PluginManifestInput,
    PluginRuleManifest, PluginRuleSetManifest, WasmPluginHost,
};
pub use project::{
    discover_move_project, AdminControlFinding, CapabilityFinding, ExternalCallFinding,
    MoveFunctionSignature, MoveModule, MovePackage, MovePackageSurface, MoveProject,
    MoveStructField, MoveStructSignature, ObjectLifecycleFunctionRef, ObjectLifecycleMap,
    ObjectLifecycleRisk, ObjectLifecycleStage, ObjectOwnershipFinding, PackageDependencyEdge,
    PackageDependencyGraph, PackageDependencyNode, PublicPackageRelationship,
};
