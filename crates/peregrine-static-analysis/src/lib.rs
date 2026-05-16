mod analyzer;
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
        AnalysisContext, AnalysisDiagnostic, AnalysisReport, Finding, Metric, ParsedFunction,
        ParsedModule, RuleMetric, Severity, SourceFile, Span,
    };
}

pub mod rules {
    pub mod complexity {
        pub use crate::sui::complexity::*;
    }

    pub mod sui {
        pub use crate::sui::rules::*;
    }
}

pub use analyzer::Analyzer;
pub use peregrine_types::analysis::{
    AnalysisConfig, AnalysisSection, PluginConfig, RuleConfig, RuleSetConfig,
};
pub use peregrine_types::analysis::{
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
    discover_project_graphs, discover_project_graphs_for_package,
    discover_state_access_graph_for_function, AdminControlFinding, CapabilityFinding,
    ExternalCallFinding, MoveCallGraph, MoveCallGraphEdge, MoveCallGraphNode,
    MoveFunctionSignature, MoveModule, MovePackage, MovePackageSurface, MoveProject,
    MoveProjectGraphs, MoveSourceSpan, MoveStateAccessGraph, MoveStateAccessGraphEdge,
    MoveStateAccessGraphNode, MoveStructField, MoveStructSignature, MoveTypeGraph,
    MoveTypeGraphEdge, MoveTypeGraphNode, MoveUnresolvedCall, MoveUnresolvedStateAccess,
    MoveUnresolvedType, ObjectLifecycleFunctionRef, ObjectLifecycleMap, ObjectLifecycleRisk,
    ObjectLifecycleStage, ObjectOwnershipFinding, PackageDependencyEdge, PackageDependencyGraph,
    PackageDependencyNode, PublicPackageRelationship,
};
