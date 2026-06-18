use peregrine_security_tools::{
    AcquiredAuditTarget, AdapterFuture, AuditAdapterError, AuditChainAdapter, AuditTargetPreflight,
    AuditWorkspace, ExploitReplay,
};
use peregrine_sui_adapter::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiExecutionTarget,
};
use peregrine_sui_import_engine::{
    BuildVerification, BuildableImportRequest, ImportEngine, ImportEngineConfig,
};
use peregrine_types::{
    AuditCapabilityBinding, AuditProfile, AuditTarget, ExploitBundle, ExploitIntent, Metadata,
    ToolDiagnostic,
};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

const ADAPTER_ID: &str = "peregrine.sui";

#[derive(Clone)]
pub struct SuiSecurityAdapter {
    settings: SuiAdapterSettings,
    environment: SuiAdapterEnvironment,
}

impl SuiSecurityAdapter {
    pub fn new(settings: SuiAdapterSettings, environment: SuiAdapterEnvironment) -> Self {
        Self {
            settings,
            environment,
        }
    }

    fn import_engine(&self, profile: &AuditProfile) -> Result<ImportEngine, AuditAdapterError> {
        let adapter = SuiAdapter::new(self.settings.clone(), self.environment.clone());
        let build_verification = match adapter.resolve() {
            Ok(SuiExecutionTarget::System { executable }) => BuildVerification::SystemSui {
                executable,
                default_move_flavor: None,
            },
            Ok(SuiExecutionTarget::Bundled) | Err(_) => BuildVerification::Disabled,
        };
        Ok(ImportEngine::new(ImportEngineConfig {
            max_dependency_depth: usize::try_from(profile.max_dependency_depth)
                .unwrap_or(usize::MAX)
                .min(16),
            max_dependency_packages: usize::try_from(profile.max_dependency_packages)
                .unwrap_or(usize::MAX)
                .clamp(1, 512),
            build_verification,
        }))
    }
}

impl Default for SuiSecurityAdapter {
    fn default() -> Self {
        Self::new(
            SuiAdapterSettings::default(),
            SuiAdapterEnvironment::default(),
        )
    }
}

impl AuditChainAdapter for SuiSecurityAdapter {
    fn adapter_id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn chain_id(&self) -> &'static str {
        "sui"
    }

    fn capabilities(&self) -> Vec<AuditCapabilityBinding> {
        [
            ("target.acquire", true, None),
            ("target.normalize", true, None),
            ("static.analysis", true, Some("MCP capability")),
            ("graph.analysis", true, Some("MCP capability")),
            ("bytecode.analysis", true, Some("MCP capability")),
            ("dynamic.fuzzing", true, Some("MCP capability")),
            ("formal.verification", true, Some("MCP capability")),
            (
                "symbolic.execution",
                false,
                Some("symbolic engine is not registered"),
            ),
            (
                "economic.simulation",
                false,
                Some("economic engine is not registered"),
            ),
            (
                "exploit.replay",
                false,
                Some("isolated PTB replay engine is not registered"),
            ),
        ]
        .into_iter()
        .map(
            |(capability, available, diagnostic)| AuditCapabilityBinding {
                capability: capability.to_string(),
                provider_id: ADAPTER_ID.to_string(),
                adapter_id: Some(ADAPTER_ID.to_string()),
                tool_name: None,
                available,
                diagnostic: diagnostic.map(str::to_string),
            },
        )
        .collect()
    }

    fn preflight<'a>(&'a self, target: &'a AuditTarget) -> AdapterFuture<'a, AuditTargetPreflight> {
        Box::pin(async move {
            let normalized_target = match target {
                AuditTarget::LocalPackage {
                    chain_id,
                    path,
                    metadata,
                } if chain_id == self.chain_id() => {
                    let root = canonical_move_package(path)?;
                    AuditTarget::LocalPackage {
                        chain_id: chain_id.clone(),
                        path: root.display().to_string(),
                        metadata: metadata.clone(),
                    }
                }
                AuditTarget::RemotePackage {
                    chain_id,
                    network_id,
                    package_ref,
                    source_uri,
                    state_ref,
                    metadata,
                } if chain_id == self.chain_id() => {
                    if network_id.trim().is_empty() || package_ref.trim().is_empty() {
                        return Err(AuditAdapterError::InvalidTarget(
                            "Sui remote targets require network_id and package_ref".to_string(),
                        ));
                    }
                    if source_uri.as_deref().is_none_or(str::is_empty) {
                        return Err(AuditAdapterError::InvalidTarget(
                            "Sui remote targets require a GraphQL source_uri".to_string(),
                        ));
                    }
                    AuditTarget::RemotePackage {
                        chain_id: chain_id.clone(),
                        network_id: network_id.trim().to_string(),
                        package_ref: package_ref.trim().to_string(),
                        source_uri: source_uri.clone(),
                        state_ref: state_ref.clone(),
                        metadata: metadata.clone(),
                    }
                }
                _ => {
                    return Err(AuditAdapterError::InvalidTarget(
                        "target does not belong to the Sui adapter".to_string(),
                    ));
                }
            };
            Ok(AuditTargetPreflight {
                adapter_id: ADAPTER_ID.to_string(),
                normalized_target,
                capabilities: self.capabilities(),
                diagnostics: Vec::new(),
            })
        })
    }

    fn acquire<'a>(
        &'a self,
        target: &'a AuditTarget,
        profile: &'a AuditProfile,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, AcquiredAuditTarget> {
        Box::pin(async move {
            match self.preflight(target).await?.normalized_target {
                AuditTarget::LocalPackage { path, .. } => {
                    let source = PathBuf::from(path);
                    let destination = workspace.input.join("package");
                    copy_package(&source, &destination)?;
                    let manifest_ref = write_manifest(
                        workspace,
                        json!({
                            "adapterId": ADAPTER_ID,
                            "kind": "localPackage",
                            "source": source,
                            "packageRoot": destination,
                        }),
                    )?;
                    Ok(AcquiredAuditTarget {
                        adapter_id: ADAPTER_ID.to_string(),
                        root: destination,
                        manifest_ref,
                        artifact_refs: Vec::new(),
                        immutable_state_ref: None,
                        diagnostics: Vec::new(),
                        metadata: Metadata::new(),
                    })
                }
                AuditTarget::RemotePackage {
                    network_id,
                    package_ref,
                    source_uri,
                    state_ref,
                    ..
                } => {
                    let import_root = workspace.input.join("imported-package");
                    let artifact = self
                        .import_engine(profile)?
                        .import_buildable_package(BuildableImportRequest {
                            network_id: network_id.clone(),
                            graph_ql_url: source_uri.ok_or_else(|| {
                                AuditAdapterError::InvalidTarget(
                                    "missing Sui GraphQL source_uri".to_string(),
                                )
                            })?,
                            package_id: package_ref,
                            import_root,
                            generate_buildable: true,
                        })
                        .await
                        .map_err(AuditAdapterError::Adapter)?;
                    let mut acquired_metadata = Metadata::new();
                    acquired_metadata.insert(
                        "importArtifact".to_string(),
                        serde_json::to_value(&artifact)
                            .map_err(|error| AuditAdapterError::Adapter(error.to_string()))?,
                    );
                    let import_metadata_ref = audit_relative_ref(
                        workspace,
                        &artifact
                            .raw_root
                            .parent()
                            .ok_or_else(|| {
                                AuditAdapterError::Adapter(
                                    "import artifact raw root has no import root".to_string(),
                                )
                            })?
                            .join(".peregrine")
                            .join("import-engine.json"),
                    )?;
                    let manifest_ref = write_manifest(
                        workspace,
                        json!({
                            "adapterId": ADAPTER_ID,
                            "kind": "remotePackage",
                            "networkId": network_id,
                            "stateRef": state_ref,
                            "importMetadataRef": import_metadata_ref,
                            "importArtifact": artifact,
                        }),
                    )?;
                    Ok(AcquiredAuditTarget {
                        adapter_id: ADAPTER_ID.to_string(),
                        root: artifact.project_root,
                        manifest_ref,
                        artifact_refs: vec![import_metadata_ref],
                        immutable_state_ref: state_ref,
                        diagnostics: artifact
                            .diagnostics
                            .into_iter()
                            .map(|diagnostic| ToolDiagnostic {
                                level: format!("{:?}", diagnostic.severity).to_lowercase(),
                                source: ADAPTER_ID.to_string(),
                                message: diagnostic.message,
                                resolution: None,
                            })
                            .collect(),
                        metadata: acquired_metadata,
                    })
                }
            }
        })
    }

    fn encode_exploit<'a>(
        &'a self,
        _target: &'a AcquiredAuditTarget,
        _intent: &'a ExploitIntent,
        _workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitBundle> {
        Box::pin(async {
            Err(AuditAdapterError::Adapter(
                "Sui exploit encoding capability is unavailable".to_string(),
            ))
        })
    }

    fn replay_exploit<'a>(
        &'a self,
        _target: &'a AcquiredAuditTarget,
        _bundle: &'a ExploitBundle,
        _workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitReplay> {
        Box::pin(async {
            Err(AuditAdapterError::Adapter(
                "Sui exploit replay capability is unavailable".to_string(),
            ))
        })
    }
}

fn canonical_move_package(path: &str) -> Result<PathBuf, AuditAdapterError> {
    let root = PathBuf::from(path)
        .canonicalize()
        .map_err(|source| AuditAdapterError::Io {
            action: "resolve local audit target",
            source,
        })?;
    if !root.join("Move.toml").is_file() {
        return Err(AuditAdapterError::InvalidTarget(format!(
            "{} does not contain Move.toml",
            root.display()
        )));
    }
    Ok(root)
}

fn copy_package(source: &Path, destination: &Path) -> Result<(), AuditAdapterError> {
    if destination.exists() {
        return Err(AuditAdapterError::InvalidTarget(format!(
            "audit input already exists at {}",
            destination.display()
        )));
    }
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| AuditAdapterError::Adapter(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| AuditAdapterError::Adapter(error.to_string()))?;
        if relative.components().next().is_some_and(|component| {
            matches!(component.as_os_str().to_str(), Some(".git" | "target"))
        }) {
            continue;
        }
        let output = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&output).map_err(|source| AuditAdapterError::Io {
                action: "create copied package directory",
                source,
            })?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|source| AuditAdapterError::Io {
                    action: "create copied package parent",
                    source,
                })?;
            }
            fs::copy(entry.path(), &output).map_err(|source| AuditAdapterError::Io {
                action: "copy local audit package",
                source,
            })?;
            let mut permissions = fs::metadata(&output)
                .map_err(|source| AuditAdapterError::Io {
                    action: "read copied package permissions",
                    source,
                })?
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&output, permissions).map_err(|source| AuditAdapterError::Io {
                action: "make copied audit input read-only",
                source,
            })?;
        }
    }
    Ok(())
}

fn write_manifest(
    workspace: &AuditWorkspace,
    value: serde_json::Value,
) -> Result<String, AuditAdapterError> {
    let path = workspace.input.join("target-manifest.json");
    let body = serde_json::to_vec_pretty(&value)
        .map_err(|error| AuditAdapterError::Adapter(error.to_string()))?;
    fs::write(&path, body).map_err(|source| AuditAdapterError::Io {
        action: "write audit target manifest",
        source,
    })?;
    audit_relative_ref(workspace, &path)
}

fn audit_relative_ref(
    workspace: &AuditWorkspace,
    path: &Path,
) -> Result<String, AuditAdapterError> {
    path.strip_prefix(&workspace.root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .map_err(|error| AuditAdapterError::Adapter(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_acquisition_copies_read_only_input() {
        let temp = tempfile::tempdir().expect("tempdir");
        let package = temp.path().join("source");
        fs::create_dir_all(package.join("sources")).expect("create package");
        fs::write(package.join("Move.toml"), "[package]\nname = \"fixture\"\n")
            .expect("write manifest");
        fs::write(package.join("sources/main.move"), "module fixture::main {}")
            .expect("write source");
        let workspace = AuditWorkspace::create(temp.path(), "audit-1").expect("create workspace");
        let target = AuditTarget::LocalPackage {
            chain_id: "sui".to_string(),
            path: package.display().to_string(),
            metadata: Metadata::new(),
        };
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let acquired = runtime
            .block_on(SuiSecurityAdapter::default().acquire(
                &target,
                &AuditProfile::default(),
                &workspace,
            ))
            .expect("acquire");

        assert!(acquired.root.join("Move.toml").is_file());
        assert_eq!(acquired.manifest_ref, "input/target-manifest.json");
        assert_eq!(acquired.artifact_refs, Vec::<String>::new());
        assert!(workspace.root.join(&acquired.manifest_ref).is_file());
        assert!(
            fs::metadata(acquired.root.join("sources/main.move"))
                .expect("metadata")
                .permissions()
                .readonly()
        );
    }
}
