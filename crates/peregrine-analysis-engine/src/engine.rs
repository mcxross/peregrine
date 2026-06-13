use crate::{AnalysisPluginRegistry, adapter_stage::prepare_target};
use peregrine_analysis::{
    AnalysisDiagnostic, AnalysisReport, AnalysisRequest, AnalysisStage, ArtifactBundle,
    DiagnosticSeverity, DynamicAnalysisOutput, DynamicResultStatus, GraphKind, PropertyGraph,
    StageReport, StageStatus,
};
use std::{collections::BTreeSet, sync::Arc, time::Instant};

#[derive(Clone)]
pub struct AnalysisEngine {
    registry: Arc<AnalysisPluginRegistry>,
}

impl AnalysisEngine {
    pub fn new(registry: AnalysisPluginRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }

    pub async fn run(&self, request: AnalysisRequest) -> AnalysisReport {
        let mut report = AnalysisReport::empty(request.chain.clone());
        let Some(adapter) = self.registry.adapter(&request.chain) else {
            let diagnostic = AnalysisDiagnostic::error(
                AnalysisStage::Adapter,
                None,
                "adapter_not_registered",
                format!(
                    "no chain adapter is registered for `{}`",
                    request.chain.as_str()
                ),
            );
            report.diagnostics.push(diagnostic.clone());
            report.stages.push(StageReport {
                stage: AnalysisStage::Adapter,
                status: StageStatus::Failed,
                plugin_ids: Vec::new(),
                diagnostics: vec![diagnostic],
                duration_ms: 0,
            });
            return report;
        };

        let descriptor = adapter.descriptor();
        report.selected_plugins.push(descriptor.id.clone());
        let started = Instant::now();
        let resolved = match adapter
            .resolve_target(&request.target, &request.limits)
            .await
        {
            Ok(resolved) => resolved,
            Err(error) => {
                let diagnostic = AnalysisDiagnostic::error(
                    AnalysisStage::Adapter,
                    Some(descriptor.id),
                    error.code,
                    error.message,
                );
                report.diagnostics.push(diagnostic.clone());
                report.stages.push(StageReport {
                    stage: AnalysisStage::Adapter,
                    status: StageStatus::Failed,
                    plugin_ids: vec![adapter.descriptor().id],
                    diagnostics: vec![diagnostic],
                    duration_ms: elapsed_ms(started),
                });
                return report;
            }
        };
        let resolved = match prepare_target(adapter.as_ref(), &request, resolved).await {
            Ok(resolved) => resolved,
            Err(error) => {
                let diagnostic = AnalysisDiagnostic::error(
                    AnalysisStage::Adapter,
                    Some(descriptor.id.clone()),
                    error.code,
                    error.message,
                );
                report.diagnostics.push(diagnostic.clone());
                report.stages.push(StageReport {
                    stage: AnalysisStage::Adapter,
                    status: StageStatus::Failed,
                    plugin_ids: vec![descriptor.id],
                    diagnostics: vec![diagnostic],
                    duration_ms: elapsed_ms(started),
                });
                return report;
            }
        };
        report.target_id = Some(resolved.target_id.clone());
        let adapter_failed = resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
        report.stages.push(StageReport {
            stage: AnalysisStage::Adapter,
            status: if adapter_failed {
                StageStatus::Failed
            } else {
                StageStatus::Passed
            },
            plugin_ids: vec![descriptor.id],
            diagnostics: resolved.diagnostics.clone(),
            duration_ms: elapsed_ms(started),
        });
        report.diagnostics.extend(resolved.diagnostics.clone());
        if adapter_failed {
            return report;
        }

        let artifacts = match self.run_scanners(&request, &resolved, &mut report).await {
            Some(artifacts) => artifacts,
            None => return report,
        };
        report.artifacts = Some(artifacts.clone());

        let graphs = if request.stages.contains(&AnalysisStage::Graph) {
            self.run_graph_builders(&request, &artifacts, &mut report)
                .await
        } else {
            report.stages.push(skipped_stage(AnalysisStage::Graph));
            Vec::new()
        };
        report.graphs = graphs.clone();

        if request.stages.contains(&AnalysisStage::Static) {
            self.run_static_analyzers(&request, &artifacts, &graphs, &mut report)
                .await;
        } else {
            report.stages.push(skipped_stage(AnalysisStage::Static));
        }

        if request.stages.contains(&AnalysisStage::Dynamic) {
            self.run_dynamic_analyzers(&request, &resolved, &artifacts, &graphs, &mut report)
                .await;
        } else {
            report.stages.push(skipped_stage(AnalysisStage::Dynamic));
        }

        report.selected_plugins.sort();
        report.selected_plugins.dedup();
        report
    }

    async fn run_scanners(
        &self,
        request: &AnalysisRequest,
        resolved: &peregrine_analysis::ResolvedTarget,
        report: &mut AnalysisReport,
    ) -> Option<ArtifactBundle> {
        if !request.stages.contains(&AnalysisStage::Scan) {
            report.stages.push(skipped_stage(AnalysisStage::Scan));
            return None;
        }

        let scanners = self
            .registry
            .scanners()
            .iter()
            .filter(|plugin| plugin.descriptor().chain == request.chain)
            .filter(|plugin| plugin_allowed(&plugin.descriptor().id, &request.plugin_ids))
            .cloned()
            .collect::<Vec<_>>();
        let started = Instant::now();
        if scanners.is_empty() {
            let diagnostic = AnalysisDiagnostic::unavailable(
                AnalysisStage::Scan,
                None,
                "scanner_not_registered",
                "no scanner plugin matched the request",
            );
            report.diagnostics.push(diagnostic.clone());
            report.stages.push(StageReport {
                stage: AnalysisStage::Scan,
                status: StageStatus::Unavailable,
                plugin_ids: Vec::new(),
                diagnostics: vec![diagnostic],
                duration_ms: elapsed_ms(started),
            });
            return None;
        }

        let mut merged: Option<ArtifactBundle> = None;
        let mut diagnostics = Vec::new();
        let mut plugin_ids = Vec::new();
        for scanner in scanners {
            let descriptor = scanner.descriptor();
            plugin_ids.push(descriptor.id.clone());
            report.selected_plugins.push(descriptor.id.clone());
            match scanner
                .scan(resolved, &request.options, &request.limits)
                .await
            {
                Ok(output) => merge_artifacts(&mut merged, output),
                Err(error) => diagnostics.push(AnalysisDiagnostic::error(
                    AnalysisStage::Scan,
                    Some(descriptor.id),
                    error.code,
                    error.message,
                )),
            }
        }

        if !diagnostics.is_empty() {
            report.diagnostics.extend(diagnostics.clone());
            report.stages.push(StageReport {
                stage: AnalysisStage::Scan,
                status: StageStatus::Failed,
                plugin_ids,
                diagnostics,
                duration_ms: elapsed_ms(started),
            });
            return None;
        }

        let mut artifacts = merged?;
        truncate_artifacts(&mut artifacts, request);
        diagnostics.extend(artifacts.diagnostics.clone());
        report.diagnostics.extend(diagnostics.clone());
        report.stages.push(StageReport {
            stage: AnalysisStage::Scan,
            status: status_for_diagnostics(&diagnostics),
            plugin_ids,
            diagnostics,
            duration_ms: elapsed_ms(started),
        });
        Some(artifacts)
    }

    async fn run_graph_builders(
        &self,
        request: &AnalysisRequest,
        artifacts: &ArtifactBundle,
        report: &mut AnalysisReport,
    ) -> Vec<PropertyGraph> {
        let started = Instant::now();
        let requested = if request.graph_kinds.is_empty() {
            GraphKind::required()
        } else {
            request.graph_kinds.clone()
        };
        let mut remaining = requested.iter().cloned().collect::<BTreeSet<_>>();
        let mut graphs = Vec::new();
        let mut diagnostics = Vec::new();
        let mut plugin_ids = Vec::new();

        let builders = self
            .registry
            .graph_builders()
            .iter()
            .filter(|plugin| {
                let descriptor = plugin.descriptor();
                descriptor.chain == request.chain
                    && plugin_allowed(&descriptor.id, &request.plugin_ids)
            })
            .cloned()
            .collect::<Vec<_>>();
        for builder in builders {
            let descriptor = builder.descriptor();
            let selected = builder
                .supported_graphs()
                .into_iter()
                .filter(|kind| remaining.contains(kind))
                .collect::<Vec<_>>();
            if selected.is_empty() {
                continue;
            }
            plugin_ids.push(descriptor.id.clone());
            report.selected_plugins.push(descriptor.id.clone());
            match builder
                .build(artifacts, &selected, &request.options, &request.limits)
                .await
            {
                Ok(mut output) => {
                    for graph in &output {
                        remaining.remove(&graph.kind);
                        diagnostics.extend(graph.diagnostics.clone());
                    }
                    truncate_graphs(&mut output, request);
                    graphs.extend(output);
                }
                Err(error) => diagnostics.push(AnalysisDiagnostic::error(
                    AnalysisStage::Graph,
                    Some(descriptor.id),
                    error.code,
                    error.message,
                )),
            }
        }

        for kind in remaining {
            diagnostics.push(AnalysisDiagnostic::unavailable(
                AnalysisStage::Graph,
                None,
                "graph_capability_unavailable",
                format!("no graph builder produced `{}`", kind.0),
            ));
        }
        let status = if plugin_ids.is_empty() {
            StageStatus::Unavailable
        } else if diagnostics.is_empty() {
            StageStatus::Passed
        } else {
            StageStatus::Partial
        };
        report.diagnostics.extend(diagnostics.clone());
        report.stages.push(StageReport {
            stage: AnalysisStage::Graph,
            status,
            plugin_ids,
            diagnostics,
            duration_ms: elapsed_ms(started),
        });
        graphs
    }

    async fn run_static_analyzers(
        &self,
        request: &AnalysisRequest,
        artifacts: &ArtifactBundle,
        graphs: &[PropertyGraph],
        report: &mut AnalysisReport,
    ) {
        let started = Instant::now();
        let mut diagnostics = Vec::new();
        let mut plugin_ids = Vec::new();
        for analyzer in self.registry.static_analyzers().iter().filter(|plugin| {
            let descriptor = plugin.descriptor();
            descriptor.chain == request.chain && plugin_allowed(&descriptor.id, &request.plugin_ids)
        }) {
            let descriptor = analyzer.descriptor();
            plugin_ids.push(descriptor.id.clone());
            report.selected_plugins.push(descriptor.id.clone());
            match analyzer
                .analyze(artifacts, graphs, &request.options, &request.limits)
                .await
            {
                Ok(output) => {
                    report.findings.extend(output.findings);
                    report.metrics.extend(output.metrics);
                    diagnostics.extend(output.diagnostics);
                }
                Err(error) => diagnostics.push(AnalysisDiagnostic::error(
                    AnalysisStage::Static,
                    Some(descriptor.id),
                    error.code,
                    error.message,
                )),
            }
        }
        let status = if plugin_ids.is_empty() {
            diagnostics.push(AnalysisDiagnostic::unavailable(
                AnalysisStage::Static,
                None,
                "static_analyzer_not_registered",
                "no static analyzer plugin matched the request",
            ));
            StageStatus::Unavailable
        } else {
            status_for_diagnostics(&diagnostics)
        };
        report.diagnostics.extend(diagnostics.clone());
        report.stages.push(StageReport {
            stage: AnalysisStage::Static,
            status,
            plugin_ids,
            diagnostics,
            duration_ms: elapsed_ms(started),
        });
    }

    async fn run_dynamic_analyzers(
        &self,
        request: &AnalysisRequest,
        resolved: &peregrine_analysis::ResolvedTarget,
        artifacts: &ArtifactBundle,
        graphs: &[PropertyGraph],
        report: &mut AnalysisReport,
    ) {
        let started = Instant::now();
        let mut diagnostics = Vec::new();
        let mut plugin_ids = Vec::new();
        for capability in &request.dynamic_capabilities {
            let analyzer = self.registry.dynamic_analyzers().iter().find(|plugin| {
                let descriptor = plugin.descriptor();
                descriptor.chain == request.chain
                    && plugin_allowed(&descriptor.id, &request.plugin_ids)
                    && descriptor
                        .capabilities
                        .iter()
                        .any(|candidate| candidate == capability)
            });
            let Some(analyzer) = analyzer else {
                let diagnostic = AnalysisDiagnostic::unavailable(
                    AnalysisStage::Dynamic,
                    None,
                    "dynamic_capability_unavailable",
                    format!("dynamic capability `{capability}` is unavailable"),
                );
                diagnostics.push(diagnostic.clone());
                report.dynamic_results.push(DynamicAnalysisOutput {
                    analyzer_id: String::new(),
                    capability: capability.clone(),
                    status: DynamicResultStatus::Unavailable,
                    result: serde_json::Value::Null,
                    evidence: Vec::new(),
                    diagnostics: vec![diagnostic],
                });
                continue;
            };
            let descriptor = analyzer.descriptor();
            plugin_ids.push(descriptor.id.clone());
            report.selected_plugins.push(descriptor.id.clone());
            match analyzer
                .analyze(
                    capability,
                    resolved,
                    artifacts,
                    graphs,
                    &request.options,
                    &request.limits,
                )
                .await
            {
                Ok(output) => {
                    diagnostics.extend(output.diagnostics.clone());
                    report.dynamic_results.push(output);
                }
                Err(error) => diagnostics.push(AnalysisDiagnostic::error(
                    AnalysisStage::Dynamic,
                    Some(descriptor.id),
                    error.code,
                    error.message,
                )),
            }
        }
        let status = if request.dynamic_capabilities.is_empty() {
            StageStatus::Skipped
        } else if plugin_ids.is_empty() {
            StageStatus::Unavailable
        } else {
            status_for_diagnostics(&diagnostics)
        };
        report.diagnostics.extend(diagnostics.clone());
        report.stages.push(StageReport {
            stage: AnalysisStage::Dynamic,
            status,
            plugin_ids,
            diagnostics,
            duration_ms: elapsed_ms(started),
        });
    }
}

fn plugin_allowed(plugin_id: &str, requested: &[String]) -> bool {
    requested.is_empty() || requested.iter().any(|candidate| candidate == plugin_id)
}

fn merge_artifacts(merged: &mut Option<ArtifactBundle>, mut output: ArtifactBundle) {
    if let Some(merged) = merged {
        merged.artifacts.append(&mut output.artifacts);
        merged.symbols.append(&mut output.symbols);
        merged.evidence.append(&mut output.evidence);
        merged.diagnostics.append(&mut output.diagnostics);
        if let (Some(left), Some(right)) =
            (merged.metadata.as_object_mut(), output.metadata.as_object())
        {
            left.extend(right.clone());
        }
    } else {
        *merged = Some(output);
    }
}

fn truncate_artifacts(artifacts: &mut ArtifactBundle, request: &AnalysisRequest) {
    artifacts.artifacts.truncate(request.limits.max_artifacts);
    artifacts
        .evidence
        .truncate(request.limits.max_evidence_items);
}

fn truncate_graphs(graphs: &mut [PropertyGraph], request: &AnalysisRequest) {
    for graph in graphs {
        graph.nodes.truncate(request.limits.max_graph_nodes);
        graph.edges.truncate(request.limits.max_graph_edges);
    }
}

fn status_for_diagnostics(diagnostics: &[AnalysisDiagnostic]) -> StageStatus {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        StageStatus::Partial
    } else {
        StageStatus::Passed
    }
}

fn skipped_stage(stage: AnalysisStage) -> StageReport {
    StageReport {
        stage,
        status: StageStatus::Skipped,
        plugin_ids: Vec::new(),
        diagnostics: Vec::new(),
        duration_ms: 0,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryError;
    use peregrine_analysis::{
        AdapterPackage, AdapterTransaction, AnalysisError, AnalysisFuture, AnalysisLimits,
        AnalysisOptions, AnalysisTarget, Artifact, ChainAdapter, ChainId, ChainOperation,
        ChainOperationResult, ExecutionEnvironment, GraphBuilder, GraphNode, PluginDescriptor,
        PluginOrigin, PluginStage, ResolvedTarget, Scanner, StaticAnalysisOutput, StaticAnalyzer,
    };
    use serde_json::json;

    #[derive(Default)]
    struct FakeAdapter {
        fail_retrieval: bool,
    }

    impl ChainAdapter for FakeAdapter {
        fn descriptor(&self) -> PluginDescriptor {
            descriptor("fake-adapter", PluginStage::Adapter, 0)
        }

        fn resolve_target<'a>(
            &'a self,
            _target: &'a AnalysisTarget,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, ResolvedTarget> {
            Box::pin(async {
                Ok(ResolvedTarget {
                    chain: ChainId::new("test"),
                    target_id: "package".to_string(),
                    package_root: None,
                    metadata: json!({}),
                    diagnostics: Vec::new(),
                })
            })
        }

        fn retrieve_package<'a>(
            &'a self,
            _target: &'a ResolvedTarget,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, AdapterPackage> {
            Box::pin(async move {
                if self.fail_retrieval {
                    return Err(AnalysisError::new(
                        "package_retrieval_failed",
                        "configured adapter retrieval failure",
                    ));
                }
                Ok(AdapterPackage {
                    id: "package".to_string(),
                    root: None,
                    bytes: Vec::new(),
                    metadata: json!({}),
                })
            })
        }

        fn retrieve_transaction<'a>(
            &'a self,
            _target: &'a ResolvedTarget,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, AdapterTransaction> {
            Box::pin(async { Err(AnalysisError::new("unused", "unused")) })
        }

        fn resolve_dependencies<'a>(
            &'a self,
            _package: &'a AdapterPackage,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, Vec<AdapterPackage>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn execution_environment<'a>(
            &'a self,
            _target: &'a ResolvedTarget,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, ExecutionEnvironment> {
            Box::pin(async { Err(AnalysisError::new("unused", "unused")) })
        }

        fn execute<'a>(
            &'a self,
            _environment: &'a ExecutionEnvironment,
            _operation: &'a ChainOperation,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, ChainOperationResult> {
            Box::pin(async { Err(AnalysisError::new("unused", "unused")) })
        }

        fn normalize_metadata(
            &self,
            metadata: serde_json::Value,
        ) -> Result<serde_json::Value, AnalysisError> {
            Ok(metadata)
        }
    }

    struct FakeScanner;

    impl Scanner for FakeScanner {
        fn descriptor(&self) -> PluginDescriptor {
            descriptor("scanner", PluginStage::Scanner, 0)
        }

        fn scan<'a>(
            &'a self,
            target: &'a ResolvedTarget,
            _options: &'a AnalysisOptions,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, ArtifactBundle> {
            Box::pin(async move {
                Ok(ArtifactBundle {
                    chain: target.chain.clone(),
                    target_id: target.target_id.clone(),
                    package_root: None,
                    artifacts: vec![Artifact {
                        id: "artifact".to_string(),
                        kind: "package".to_string(),
                        name: "package".to_string(),
                        path: None,
                        metadata: json!({}),
                    }],
                    symbols: Vec::new(),
                    evidence: Vec::new(),
                    diagnostics: Vec::new(),
                    metadata: json!({}),
                })
            })
        }
    }

    struct FakeGraphBuilder {
        id: &'static str,
        priority: i32,
        fail: bool,
    }

    impl GraphBuilder for FakeGraphBuilder {
        fn descriptor(&self) -> PluginDescriptor {
            descriptor(self.id, PluginStage::GraphBuilder, self.priority)
        }

        fn supported_graphs(&self) -> Vec<GraphKind> {
            vec![GraphKind::new(GraphKind::CALL)]
        }

        fn build<'a>(
            &'a self,
            _artifacts: &'a ArtifactBundle,
            _requested: &'a [GraphKind],
            _options: &'a AnalysisOptions,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, Vec<PropertyGraph>> {
            Box::pin(async move {
                if self.fail {
                    return Err(AnalysisError::new(
                        "graph_failed",
                        "configured graph failure",
                    ));
                }
                Ok(vec![PropertyGraph {
                    kind: GraphKind::new(GraphKind::CALL),
                    nodes: vec![GraphNode {
                        id: self.id.to_string(),
                        kind: "function".to_string(),
                        label: self.id.to_string(),
                        span: None,
                        metadata: json!({}),
                    }],
                    edges: Vec::new(),
                    diagnostics: Vec::new(),
                    metadata: json!({}),
                }])
            })
        }
    }

    struct FakeStaticAnalyzer;

    impl StaticAnalyzer for FakeStaticAnalyzer {
        fn descriptor(&self) -> PluginDescriptor {
            descriptor("static", PluginStage::StaticAnalyzer, 0)
        }

        fn analyze<'a>(
            &'a self,
            _artifacts: &'a ArtifactBundle,
            _graphs: &'a [PropertyGraph],
            _options: &'a AnalysisOptions,
            _limits: &'a AnalysisLimits,
        ) -> AnalysisFuture<'a, StaticAnalysisOutput> {
            Box::pin(async { Ok(StaticAnalysisOutput::default()) })
        }
    }

    fn descriptor(id: &str, stage: PluginStage, priority: i32) -> PluginDescriptor {
        PluginDescriptor {
            id: id.to_string(),
            version: "1".to_string(),
            chain: ChainId::new("test"),
            stage,
            capabilities: Vec::new(),
            origin: PluginOrigin::BuiltIn,
            priority,
        }
    }

    fn registry() -> AnalysisPluginRegistry {
        let mut registry = AnalysisPluginRegistry::default();
        assert_eq!(
            registry.register_adapter(Arc::new(FakeAdapter::default())),
            Ok(())
        );
        registry.register_scanner(Arc::new(FakeScanner));
        registry.register_static_analyzer(Arc::new(FakeStaticAnalyzer));
        registry
    }

    #[test]
    fn duplicate_chain_adapters_are_rejected() {
        let mut registry = AnalysisPluginRegistry::default();
        assert_eq!(
            registry.register_adapter(Arc::new(FakeAdapter::default())),
            Ok(())
        );
        assert_eq!(
            registry.register_adapter(Arc::new(FakeAdapter::default())),
            Err(RegistryError {
                message: "chain adapter for `test` is already registered".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn higher_priority_plugins_run_first() {
        let mut registry = registry();
        registry.register_graph_builder(Arc::new(FakeGraphBuilder {
            id: "low",
            priority: 1,
            fail: false,
        }));
        registry.register_graph_builder(Arc::new(FakeGraphBuilder {
            id: "high",
            priority: 10,
            fail: false,
        }));
        let request = AnalysisRequest {
            graph_kinds: vec![GraphKind::new(GraphKind::CALL)],
            ..AnalysisRequest::safe(
                ChainId::new("test"),
                AnalysisTarget::LocalPackage { path: ".".into() },
            )
        };

        let report = AnalysisEngine::new(registry).run(request).await;

        assert_eq!(report.graphs[0].nodes[0].id, "high");
    }

    #[tokio::test]
    async fn missing_dynamic_capabilities_are_visible() {
        let registry = registry();
        let mut request = AnalysisRequest::safe(
            ChainId::new("test"),
            AnalysisTarget::LocalPackage { path: ".".into() },
        );
        request.stages.push(AnalysisStage::Dynamic);
        request.dynamic_capabilities = vec!["symbolicExecution".to_string()];

        let report = AnalysisEngine::new(registry).run(request).await;

        assert_eq!(
            report.dynamic_results[0].status,
            DynamicResultStatus::Unavailable
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "dynamic_capability_unavailable")
        );
    }

    #[tokio::test]
    async fn adapter_retrieval_failure_blocks_dependent_stages() {
        let mut registry = AnalysisPluginRegistry::default();
        assert_eq!(
            registry.register_adapter(Arc::new(FakeAdapter {
                fail_retrieval: true,
            })),
            Ok(())
        );
        registry.register_scanner(Arc::new(FakeScanner));

        let report = AnalysisEngine::new(registry)
            .run(AnalysisRequest::safe(
                ChainId::new("test"),
                AnalysisTarget::LocalPackage { path: ".".into() },
            ))
            .await;

        assert_eq!(report.stages.len(), 1);
        assert_eq!(report.stages[0].stage, AnalysisStage::Adapter);
        assert_eq!(report.stages[0].status, StageStatus::Failed);
        assert!(report.artifacts.is_none());
    }

    #[tokio::test]
    async fn graph_plugin_failure_produces_partial_report_and_static_continues() {
        let mut registry = registry();
        registry.register_graph_builder(Arc::new(FakeGraphBuilder {
            id: "failing-graph",
            priority: 1,
            fail: true,
        }));
        let request = AnalysisRequest {
            graph_kinds: vec![GraphKind::new(GraphKind::CALL)],
            ..AnalysisRequest::safe(
                ChainId::new("test"),
                AnalysisTarget::LocalPackage { path: ".".into() },
            )
        };

        let report = AnalysisEngine::new(registry).run(request).await;
        let Some(graph_stage) = report
            .stages
            .iter()
            .find(|stage| stage.stage == AnalysisStage::Graph)
        else {
            panic!("graph stage was not reported");
        };
        let Some(static_stage) = report
            .stages
            .iter()
            .find(|stage| stage.stage == AnalysisStage::Static)
        else {
            panic!("static stage was not reported");
        };

        assert_eq!(graph_stage.status, StageStatus::Partial);
        assert_eq!(static_stage.status, StageStatus::Passed);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "graph_failed")
        );
    }
}
