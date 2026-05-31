use crate::{
    output::{
        CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep, EXIT_SUCCESS,
        EXIT_WORKFLOW_FAILED, elapsed_ms,
    },
    sui::{args::ImportPackageArgs, project::resolve_output_path},
};
use peregrine_import_engine::sui::{
    BuildVerification, BuildableImportArtifact, BuildableImportRequest, EngineDiagnostic,
    EngineDiagnosticSeverity, ImportEngine, ImportEngineConfig, default_import_root,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Instant};

pub fn run_import_package(workspace_root: &Path, args: &ImportPackageArgs) -> CliStep {
    let started_at = Instant::now();
    let import_root = match args.output.as_deref() {
        Some(output) => resolve_output_path(workspace_root, Some(output)),
        None => match default_import_root(workspace_root, args.network.id(), &args.package_id) {
            Ok(path) => path,
            Err(error) => {
                return CliStep::failed(
                    "import-package",
                    started_at,
                    CliDiagnostic::error("import-package", error),
                );
            }
        },
    };
    let request = BuildableImportRequest {
        network_id: args.network.id().to_string(),
        graph_ql_url: args.network.graph_ql_url().to_string(),
        package_id: args.package_id.clone(),
        import_root: import_root.clone(),
        generate_buildable: !args.raw_only,
    };
    let engine = ImportEngine::new(ImportEngineConfig {
        max_dependency_depth: args.max_dependency_depth,
        max_dependency_packages: args.max_dependency_packages,
        build_verification: BuildVerification::Disabled,
    });
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return CliStep::failed(
                "import-package",
                started_at,
                CliDiagnostic::error("runtime", error.to_string()),
            );
        }
    };

    match runtime.block_on(engine.import_buildable_package(request)) {
        Ok(artifact) => import_success_step(started_at, args, import_root, artifact),
        Err(error) => CliStep::failed(
            "import-package",
            started_at,
            CliDiagnostic::error("import-package", error),
        ),
    }
}

fn import_success_step(
    started_at: Instant,
    args: &ImportPackageArgs,
    import_root: std::path::PathBuf,
    artifact: BuildableImportArtifact,
) -> CliStep {
    let diagnostics = artifact
        .diagnostics
        .iter()
        .map(map_engine_diagnostic)
        .collect::<Vec<_>>();
    let has_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == CliDiagnosticSeverity::Error);
    let status = if has_error {
        CliStatus::Failed
    } else {
        CliStatus::Passed
    };

    CliStep {
        name: "import-package".to_string(),
        status,
        duration_ms: elapsed_ms(started_at),
        exit_code: if status == CliStatus::Passed {
            EXIT_SUCCESS
        } else {
            EXIT_WORKFLOW_FAILED
        },
        command: Some(format!(
            "peregrine import-package --network {} --package-id {}",
            args.network.id(),
            args.package_id
        )),
        diagnostics,
        metadata: BTreeMap::from([
            (
                "network".to_string(),
                Value::String(args.network.id().to_string()),
            ),
            (
                "graphQlUrl".to_string(),
                Value::String(args.network.graph_ql_url().to_string()),
            ),
            (
                "packageId".to_string(),
                Value::String(args.package_id.clone()),
            ),
            (
                "importRoot".to_string(),
                Value::String(import_root.display().to_string()),
            ),
            ("generateBuildable".to_string(), json!(!args.raw_only)),
            (
                "dependencyCount".to_string(),
                json!(artifact.dependencies.len()),
            ),
            (
                "diagnosticCount".to_string(),
                json!(artifact.diagnostics.len()),
            ),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        details: json!({ "artifact": artifact }),
    }
}

fn map_engine_diagnostic(diagnostic: &EngineDiagnostic) -> CliDiagnostic {
    CliDiagnostic {
        severity: match diagnostic.severity {
            EngineDiagnosticSeverity::Info => CliDiagnosticSeverity::Info,
            EngineDiagnosticSeverity::Warning => CliDiagnosticSeverity::Warning,
            EngineDiagnosticSeverity::Error => CliDiagnosticSeverity::Error,
        },
        source: format!("import:{}", diagnostic.stage),
        code: None,
        message: diagnostic.message.clone(),
        file: diagnostic.module.clone(),
        span: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sui::args::ImportNetwork;
    use tempfile::tempdir;

    #[test]
    fn import_package_rejects_invalid_package_id_before_network_call() {
        let temp = tempdir().expect("tempdir");
        let step = run_import_package(
            temp.path(),
            &ImportPackageArgs {
                network: ImportNetwork::Testnet,
                package_id: "not-a-package-id".to_string(),
                output: None,
                raw_only: false,
                max_dependency_depth: 3,
                max_dependency_packages: 64,
            },
        );

        assert_eq!(step.status, CliStatus::Failed);
        assert_eq!(step.diagnostics[0].source, "import-package");
    }
}
