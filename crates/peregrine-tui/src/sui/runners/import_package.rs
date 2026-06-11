use crate::{
    output::{
        CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep, EXIT_SUCCESS,
        EXIT_WORKFLOW_FAILED, elapsed_ms,
    },
    session::McpToolClient,
    sui::args::ImportPackageArgs,
};
use peregrine_mcp_protocol::{
    ImportDiagnostic, ImportDiagnosticSeverity, ImportPackageArgs as McpImportPackageArgs,
    ImportPackageResponse, tool_name,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Instant};

pub fn run_import_package(workspace_root: &Path, args: &ImportPackageArgs) -> CliStep {
    let started_at = Instant::now();
    let request = McpImportPackageArgs {
        project_root: Some(workspace_root.display().to_string()),
        network_id: args.network.id().to_string(),
        graph_ql_url: args.network.graph_ql_url().to_string(),
        package_id: args.package_id.clone(),
        output_path: args
            .output
            .as_deref()
            .map(|path| path.display().to_string()),
        raw_only: args.raw_only,
        max_dependency_depth: Some(args.max_dependency_depth),
        max_dependency_packages: Some(args.max_dependency_packages),
    };

    match McpToolClient::call_blocking::<_, ImportPackageResponse>(
        workspace_root,
        tool_name::IMPORT_PACKAGE,
        &request,
    ) {
        Ok(response) => import_success_step(started_at, args, response),
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
    response: ImportPackageResponse,
) -> CliStep {
    let artifact = response.artifact;
    let diagnostics = artifact
        .diagnostics
        .iter()
        .map(map_import_diagnostic)
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
                Value::String(response.import_root),
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

fn map_import_diagnostic(diagnostic: &ImportDiagnostic) -> CliDiagnostic {
    CliDiagnostic {
        severity: match diagnostic.severity {
            ImportDiagnosticSeverity::Info => CliDiagnosticSeverity::Info,
            ImportDiagnosticSeverity::Warning => CliDiagnosticSeverity::Warning,
            ImportDiagnosticSeverity::Error => CliDiagnosticSeverity::Error,
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
