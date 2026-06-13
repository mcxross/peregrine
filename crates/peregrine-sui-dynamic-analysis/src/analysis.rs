use crate::{
    FormalVerificationOptions, MovyFuzzOptions, run_formal_verification_blocking,
    run_movy_fuzz_blocking,
};
use peregrine_analysis::{
    AnalysisDiagnostic, AnalysisFuture, AnalysisLimits, AnalysisOptions, AnalysisStage,
    ArtifactBundle, ChainId, DynamicAnalysisOutput, DynamicAnalyzer, DynamicResultStatus,
    PluginDescriptor, PluginOrigin, PluginStage, PropertyGraph, ResolvedTarget,
};
use serde_json::{Value, json};
use std::path::PathBuf;

const MOVY_PLUGIN_ID: &str = "peregrine.sui.movy";
const PROVER_PLUGIN_ID: &str = "peregrine.sui.prover";

#[derive(Default)]
pub struct MovyDynamicAnalyzer;

impl DynamicAnalyzer for MovyDynamicAnalyzer {
    fn descriptor(&self) -> PluginDescriptor {
        dynamic_descriptor(MOVY_PLUGIN_ID, "fuzzing")
    }

    fn analyze<'a>(
        &'a self,
        capability: &'a str,
        target: &'a ResolvedTarget,
        artifacts: &'a ArtifactBundle,
        _graphs: &'a [PropertyGraph],
        options: &'a AnalysisOptions,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, DynamicAnalysisOutput> {
        Box::pin(async move {
            if capability != "fuzzing" {
                return Ok(unavailable(
                    MOVY_PLUGIN_ID,
                    capability,
                    "Movy only provides the `fuzzing` capability",
                ));
            }
            let (project_root, package_path) = execution_paths(target, artifacts, options);
            let fuzz_options = MovyFuzzOptions {
                time_limit_seconds: integer_option(options, "timeLimitSeconds").unwrap_or(30),
                seed: integer_option(options, "seed").unwrap_or(1),
            };
            let run = tokio::task::spawn_blocking(move || {
                run_movy_fuzz_blocking(project_root, &package_path, fuzz_options)
            })
            .await;
            match run {
                Ok(Ok(run)) => Ok(DynamicAnalysisOutput {
                    analyzer_id: MOVY_PLUGIN_ID.to_string(),
                    capability: capability.to_string(),
                    status: DynamicResultStatus::Completed,
                    result: serde_json::to_value(run).unwrap_or_else(|_| json!({})),
                    evidence: Vec::new(),
                    diagnostics: Vec::new(),
                }),
                Ok(Err(error)) => Ok(failed(MOVY_PLUGIN_ID, capability, error.to_string())),
                Err(error) => Ok(failed(
                    MOVY_PLUGIN_ID,
                    capability,
                    format!("Movy worker failed: {error}"),
                )),
            }
        })
    }
}

#[derive(Default)]
pub struct SuiProverDynamicAnalyzer;

impl DynamicAnalyzer for SuiProverDynamicAnalyzer {
    fn descriptor(&self) -> PluginDescriptor {
        dynamic_descriptor(PROVER_PLUGIN_ID, "formalVerification")
    }

    fn analyze<'a>(
        &'a self,
        capability: &'a str,
        target: &'a ResolvedTarget,
        artifacts: &'a ArtifactBundle,
        _graphs: &'a [PropertyGraph],
        options: &'a AnalysisOptions,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, DynamicAnalysisOutput> {
        Box::pin(async move {
            if capability != "formalVerification" {
                return Ok(unavailable(
                    PROVER_PLUGIN_ID,
                    capability,
                    "Sui Prover only provides the `formalVerification` capability",
                ));
            }
            let module_name = string_option(options, "moduleName");
            let file_path = string_option(options, "filePath");
            if module_name.is_empty() || file_path.is_empty() {
                return Ok(failed(
                    PROVER_PLUGIN_ID,
                    capability,
                    "formal verification requires `moduleName` and `filePath` options".to_string(),
                ));
            }
            let (project_root, package_path) = execution_paths(target, artifacts, options);
            let timeout_seconds = integer_option(options, "timeoutSeconds")
                .and_then(|value| usize::try_from(value).ok());
            let verification_options = FormalVerificationOptions {
                module_name,
                file_path,
                timeout_seconds,
                verbose: bool_option(options, "verbose"),
                trace: bool_option(options, "trace"),
                keep_temp: bool_option(options, "keepTemp"),
            };
            let run = tokio::task::spawn_blocking(move || {
                run_formal_verification_blocking(project_root, &package_path, verification_options)
            })
            .await;
            match run {
                Ok(Ok(run)) => Ok(DynamicAnalysisOutput {
                    analyzer_id: PROVER_PLUGIN_ID.to_string(),
                    capability: capability.to_string(),
                    status: DynamicResultStatus::Completed,
                    result: serde_json::to_value(run).unwrap_or_else(|_| json!({})),
                    evidence: Vec::new(),
                    diagnostics: Vec::new(),
                }),
                Ok(Err(error)) => Ok(failed(PROVER_PLUGIN_ID, capability, error.to_string())),
                Err(error) => Ok(failed(
                    PROVER_PLUGIN_ID,
                    capability,
                    format!("Sui Prover worker failed: {error}"),
                )),
            }
        })
    }
}

fn dynamic_descriptor(id: &str, capability: &str) -> PluginDescriptor {
    PluginDescriptor {
        id: id.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        chain: ChainId::new("sui"),
        stage: PluginStage::DynamicAnalyzer,
        capabilities: vec![capability.to_string()],
        origin: PluginOrigin::BuiltIn,
        priority: 100,
    }
}

fn execution_paths(
    target: &ResolvedTarget,
    artifacts: &ArtifactBundle,
    options: &AnalysisOptions,
) -> (PathBuf, String) {
    let package_root = artifacts
        .package_root
        .as_ref()
        .or(target.package_root.as_ref())
        .cloned()
        .unwrap_or_default();
    let project_root = options
        .get("projectRoot")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            target
                .metadata
                .get("projectRoot")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| package_root.clone());
    let package_path = options
        .get("packagePath")
        .and_then(Value::as_str)
        .or_else(|| target.metadata.get("packagePath").and_then(Value::as_str))
        .unwrap_or(".")
        .to_string();
    (project_root, package_path)
}

fn integer_option(options: &AnalysisOptions, key: &str) -> Option<u64> {
    options.get(key).and_then(Value::as_u64)
}

fn string_option(options: &AnalysisOptions, key: &str) -> String {
    options
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn bool_option(options: &AnalysisOptions, key: &str) -> bool {
    options.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn unavailable(plugin_id: &str, capability: &str, message: &str) -> DynamicAnalysisOutput {
    let diagnostic = AnalysisDiagnostic::unavailable(
        AnalysisStage::Dynamic,
        Some(plugin_id.to_string()),
        "dynamic_capability_unavailable",
        message,
    );
    DynamicAnalysisOutput {
        analyzer_id: plugin_id.to_string(),
        capability: capability.to_string(),
        status: DynamicResultStatus::Unavailable,
        result: Value::Null,
        evidence: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

fn failed(plugin_id: &str, capability: &str, message: String) -> DynamicAnalysisOutput {
    let diagnostic = AnalysisDiagnostic::error(
        AnalysisStage::Dynamic,
        Some(plugin_id.to_string()),
        "dynamic_analysis_failed",
        message,
    );
    DynamicAnalysisOutput {
        analyzer_id: plugin_id.to_string(),
        capability: capability.to_string(),
        status: DynamicResultStatus::Failed,
        result: Value::Null,
        evidence: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}
