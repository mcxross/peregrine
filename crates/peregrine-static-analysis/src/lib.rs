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
    AnalyzerPluginRegistry, AnalyzerPluginRegistryFile, InstalledAnalyzerPlugin,
    PluginActiveRuleConfig, PluginAnalyzeInput, PluginAnalyzeOutput, PluginManifest,
    PluginManifestInput, PluginRuleManifest, PluginRuleSetManifest, WasmPluginHost,
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
