mod analyzer;
mod parser;
mod plugins;
mod project;
pub mod sui;

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

    pub mod sui {
        pub use crate::sui::*;
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
    discover_move_project, discover_move_project_fast, discover_move_project_shallow,
    discover_project_graphs, discover_project_graphs_for_package, AdminControlFinding,
    CapabilityFinding, ExternalCallFinding, MoveCallGraph, MoveCallGraphEdge, MoveCallGraphNode,
    MoveFunctionSignature, MoveModule, MovePackage, MovePackageSurface, MoveProject,
    MoveProjectGraphs, MoveSourceSpan, MoveStructField, MoveStructSignature, MoveTypeGraph,
    MoveTypeGraphEdge, MoveTypeGraphNode, MoveUnresolvedCall, MoveUnresolvedType,
    ObjectLifecycleFunctionRef, ObjectLifecycleMap, ObjectLifecycleRisk, ObjectLifecycleStage,
    ObjectOwnershipFinding, PackageDependencyEdge, PackageDependencyGraph, PackageDependencyNode,
    PublicPackageRelationship,
};
