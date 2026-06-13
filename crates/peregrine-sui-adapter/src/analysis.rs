use crate::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiAdapterSource, SuiCommandKind,
};
use peregrine_analysis::{
    AdapterPackage, AdapterTransaction, AnalysisDiagnostic, AnalysisError, AnalysisFuture,
    AnalysisLimits, AnalysisStage, AnalysisTarget, ChainAdapter, ChainId, ChainOperation,
    ChainOperationResult, ExecutionEnvironment, PluginDescriptor, PluginOrigin, PluginStage,
    ResolvedTarget,
};
use peregrine_sui_package_resolution::{
    fetch_move_package_from_graphql, normalize_sui_package_id, validated_graphql_url,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

const PLUGIN_ID: &str = "peregrine.sui.adapter";

pub struct SuiChainAdapter {
    adapter: Arc<SuiAdapter>,
}

impl Default for SuiChainAdapter {
    fn default() -> Self {
        Self::new(SuiAdapterSettings::default())
    }
}

impl SuiChainAdapter {
    pub fn new(settings: SuiAdapterSettings) -> Self {
        Self {
            adapter: Arc::new(SuiAdapter::new(settings, SuiAdapterEnvironment::default())),
        }
    }
}

impl ChainAdapter for SuiChainAdapter {
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: PLUGIN_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            chain: ChainId::new("sui"),
            stage: PluginStage::Adapter,
            capabilities: vec![
                "localPackage".to_string(),
                "onChainPackage".to_string(),
                "dependencyResolution".to_string(),
                "boundedExecution".to_string(),
            ],
            origin: PluginOrigin::BuiltIn,
            priority: 100,
        }
    }

    fn resolve_target<'a>(
        &'a self,
        target: &'a AnalysisTarget,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ResolvedTarget> {
        Box::pin(async move {
            match target {
                AnalysisTarget::LocalPackage { path } => resolve_local_target(path),
                AnalysisTarget::OnChainPackage {
                    network,
                    package_id,
                    endpoint,
                } => resolve_on_chain_target(network, package_id, endpoint.as_deref()),
                AnalysisTarget::Transaction {
                    network,
                    digest,
                    endpoint,
                } => Ok(ResolvedTarget {
                    chain: ChainId::new("sui"),
                    target_id: digest.trim().to_string(),
                    package_root: None,
                    metadata: json!({
                        "targetKind": "transaction",
                        "network": network,
                        "endpoint": endpoint,
                    }),
                    diagnostics: Vec::new(),
                }),
            }
        })
    }

    fn retrieve_package<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, AdapterPackage> {
        Box::pin(async move {
            if let Some(root) = &target.package_root {
                let manifest_path = root.join("Move.toml");
                let bytes = fs::read(&manifest_path).map_err(|error| {
                    AnalysisError::new(
                        "package_read_failed",
                        format!("could not read {}: {error}", manifest_path.display()),
                    )
                })?;
                return Ok(AdapterPackage {
                    id: target.target_id.clone(),
                    root: Some(root.clone()),
                    bytes,
                    metadata: target.metadata.clone(),
                });
            }

            let endpoint = target
                .metadata
                .get("endpoint")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AnalysisError::new(
                        "missing_package_endpoint",
                        "on-chain Sui package target has no GraphQL endpoint",
                    )
                })?;
            let package = fetch_move_package_from_graphql(endpoint, &target.target_id)
                .await
                .map_err(|message| AnalysisError::new("package_retrieval_failed", message))?;
            let bytes = serde_json::to_vec(
                &package
                    .modules
                    .iter()
                    .map(|module| (&module.name, &module.bytecode))
                    .collect::<BTreeMap<_, _>>(),
            )
            .map_err(|error| {
                AnalysisError::new(
                    "package_serialization_failed",
                    format!("could not serialize retrieved package: {error}"),
                )
            })?;

            Ok(AdapterPackage {
                id: package.address,
                root: None,
                bytes,
                metadata: json!({
                    "network": target.metadata.get("network"),
                    "endpoint": endpoint,
                    "version": package.version,
                    "digest": package.digest,
                    "modules": package.modules.iter().map(|module| &module.name).collect::<Vec<_>>(),
                }),
            })
        })
    }

    fn retrieve_transaction<'a>(
        &'a self,
        _target: &'a ResolvedTarget,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, AdapterTransaction> {
        Box::pin(async {
            Err(AnalysisError::new(
                "transaction_access_unavailable",
                "Sui transaction retrieval is not implemented by this adapter",
            ))
        })
    }

    fn resolve_dependencies<'a>(
        &'a self,
        package: &'a AdapterPackage,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, Vec<AdapterPackage>> {
        Box::pin(async move {
            let Some(root) = &package.root else {
                return Ok(Vec::new());
            };
            let manifest = fs::read_to_string(root.join("Move.toml")).map_err(|error| {
                AnalysisError::new(
                    "manifest_read_failed",
                    format!("could not read Move.toml: {error}"),
                )
            })?;
            let manifest = toml::from_str::<toml::Value>(&manifest).map_err(|error| {
                AnalysisError::new(
                    "manifest_parse_failed",
                    format!("could not parse Move.toml: {error}"),
                )
            })?;
            let dependencies = manifest
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|dependencies| dependencies.keys())
                .map(|name| AdapterPackage {
                    id: name.clone(),
                    root: None,
                    bytes: Vec::new(),
                    metadata: json!({"declaredBy": package.id}),
                })
                .collect();
            Ok(dependencies)
        })
    }

    fn execution_environment<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ExecutionEnvironment> {
        Box::pin(async move {
            let package_root = target.package_root.as_ref().ok_or_else(|| {
                AnalysisError::new(
                    "execution_target_unavailable",
                    "Sui execution currently requires a local package",
                )
            })?;
            Ok(ExecutionEnvironment {
                id: format!("sui:{}", target.target_id),
                metadata: json!({"packageRoot": package_root}),
            })
        })
    }

    fn execute<'a>(
        &'a self,
        environment: &'a ExecutionEnvironment,
        operation: &'a ChainOperation,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ChainOperationResult> {
        Box::pin(async move {
            let package_root = environment
                .metadata
                .get("packageRoot")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AnalysisError::new(
                        "invalid_execution_environment",
                        "Sui execution environment has no package root",
                    )
                })?;
            let kind = match operation.kind.as_str() {
                "build" => SuiCommandKind::MoveBuild,
                "test" => SuiCommandKind::MoveTest,
                "coverage" => SuiCommandKind::MoveCoverage,
                "coverageSummary" => SuiCommandKind::MoveCoverageSummary,
                "fuzz" => SuiCommandKind::MoveFuzz,
                other => {
                    return Err(AnalysisError::new(
                        "unsupported_chain_operation",
                        format!("unsupported Sui operation `{other}`"),
                    ));
                }
            };
            let command = self
                .adapter
                .package_command_for(
                    kind, /*publish_build_env*/ None,
                    /*with_unpublished_dependencies*/ false,
                )
                .map_err(|error| {
                    AnalysisError::new("command_resolution_failed", error.to_string())
                })?;
            let output = match command.source() {
                SuiAdapterSource::Bundled => command.run_bundled_blocking(Path::new(package_root)),
                SuiAdapterSource::System => command.run_system_blocking(Path::new(package_root)),
            }
            .map_err(|error| AnalysisError::new("chain_operation_failed", error.to_string()))?;
            let succeeded = output.status == Some(0);
            let diagnostics = (!succeeded)
                .then(|| {
                    AnalysisDiagnostic::error(
                        AnalysisStage::Adapter,
                        Some(PLUGIN_ID.to_string()),
                        "chain_operation_failed",
                        output.stderr.clone(),
                    )
                })
                .into_iter()
                .collect();

            Ok(ChainOperationResult {
                status: if succeeded { "completed" } else { "failed" }.to_string(),
                output: json!({
                    "exitCode": output.status,
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                }),
                diagnostics,
            })
        })
    }

    fn normalize_metadata(&self, metadata: Value) -> Result<Value, AnalysisError> {
        if metadata.is_object() {
            Ok(metadata)
        } else {
            Err(AnalysisError::new(
                "invalid_metadata",
                "normalized Sui metadata must be a JSON object",
            ))
        }
    }
}

fn resolve_local_target(path: &Path) -> Result<ResolvedTarget, AnalysisError> {
    let package_root = path.canonicalize().map_err(|error| {
        AnalysisError::new(
            "package_not_found",
            format!("could not open local package {}: {error}", path.display()),
        )
    })?;
    if !package_root.is_dir() || !package_root.join("Move.toml").is_file() {
        return Err(AnalysisError::new(
            "invalid_move_package",
            format!("{} does not contain Move.toml", package_root.display()),
        ));
    }
    let manifest = fs::read_to_string(package_root.join("Move.toml")).map_err(|error| {
        AnalysisError::new(
            "manifest_read_failed",
            format!("could not read Move.toml: {error}"),
        )
    })?;
    let package_name = toml::from_str::<toml::Value>(&manifest)
        .ok()
        .and_then(|manifest| {
            manifest
                .get("package")
                .and_then(toml::Value::as_table)
                .and_then(|package| package.get("name"))
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            package_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package")
                .to_string()
        });

    Ok(ResolvedTarget {
        chain: ChainId::new("sui"),
        target_id: format!("local:{package_name}"),
        package_root: Some(package_root.clone()),
        metadata: json!({
            "targetKind": "localPackage",
            "projectRoot": package_root,
            "packagePath": ".",
            "packageName": package_name,
        }),
        diagnostics: Vec::new(),
    })
}

fn resolve_on_chain_target(
    network: &str,
    package_id: &str,
    endpoint: Option<&str>,
) -> Result<ResolvedTarget, AnalysisError> {
    let package_id = normalize_sui_package_id(package_id)
        .map_err(|message| AnalysisError::new("invalid_package_id", message))?;
    let endpoint = endpoint
        .map(validated_graphql_url)
        .transpose()
        .map_err(|message| AnalysisError::new("invalid_graphql_endpoint", message))?
        .unwrap_or_else(|| default_graphql_endpoint(network).to_string());

    Ok(ResolvedTarget {
        chain: ChainId::new("sui"),
        target_id: package_id,
        package_root: None,
        metadata: json!({
            "targetKind": "onChainPackage",
            "network": network,
            "endpoint": endpoint,
        }),
        diagnostics: vec![AnalysisDiagnostic::unavailable(
            AnalysisStage::Adapter,
            Some(PLUGIN_ID.to_string()),
            "package_not_materialized",
            "on-chain package metadata is resolved; source analysis requires package materialization",
        )],
    })
}

fn default_graphql_endpoint(network: &str) -> &'static str {
    match network.trim().to_ascii_lowercase().as_str() {
        "testnet" => "https://sui-testnet.mystenlabs.com/graphql",
        "devnet" => "https://sui-devnet.mystenlabs.com/graphql",
        _ => "https://sui-mainnet.mystenlabs.com/graphql",
    }
}
