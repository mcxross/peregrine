use crate::{
    AdapterPackage, AdapterTransaction, AnalysisError, AnalysisLimits, AnalysisOptions,
    AnalysisTarget, ArtifactBundle, ChainOperation, ChainOperationResult, DynamicAnalysisOutput,
    ExecutionEnvironment, GraphKind, PluginDescriptor, PropertyGraph, ResolvedTarget,
    StaticAnalysisOutput,
};
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub type AnalysisFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, AnalysisError>> + Send + 'a>>;

/// Encapsulates chain-specific retrieval, dependency, execution, and metadata behavior.
pub trait ChainAdapter: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;

    fn resolve_target<'a>(
        &'a self,
        target: &'a AnalysisTarget,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ResolvedTarget>;

    fn retrieve_package<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, AdapterPackage>;

    fn retrieve_transaction<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, AdapterTransaction>;

    fn resolve_dependencies<'a>(
        &'a self,
        package: &'a AdapterPackage,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, Vec<AdapterPackage>>;

    fn execution_environment<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ExecutionEnvironment>;

    fn execute<'a>(
        &'a self,
        environment: &'a ExecutionEnvironment,
        operation: &'a ChainOperation,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ChainOperationResult>;

    fn normalize_metadata(&self, metadata: Value) -> Result<Value, AnalysisError>;
}

/// Discovers chain artifacts and converts them into the canonical analysis model.
pub trait Scanner: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;

    fn scan<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        options: &'a AnalysisOptions,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ArtifactBundle>;
}

/// Builds one or more property graphs from normalized artifacts.
pub trait GraphBuilder: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;
    fn supported_graphs(&self) -> Vec<GraphKind>;

    fn build<'a>(
        &'a self,
        artifacts: &'a ArtifactBundle,
        requested: &'a [GraphKind],
        options: &'a AnalysisOptions,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, Vec<PropertyGraph>>;
}

/// Runs rule-based or graph-based analysis over normalized inputs.
pub trait StaticAnalyzer: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;

    fn analyze<'a>(
        &'a self,
        artifacts: &'a ArtifactBundle,
        graphs: &'a [PropertyGraph],
        options: &'a AnalysisOptions,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, StaticAnalysisOutput>;
}

/// Runs one dynamic capability such as fuzzing, simulation, or formal verification.
pub trait DynamicAnalyzer: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;

    fn analyze<'a>(
        &'a self,
        capability: &'a str,
        target: &'a ResolvedTarget,
        artifacts: &'a ArtifactBundle,
        graphs: &'a [PropertyGraph],
        options: &'a AnalysisOptions,
        limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, DynamicAnalysisOutput>;
}
