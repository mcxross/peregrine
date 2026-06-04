use crate::{
    output::{CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep, EXIT_WORKFLOW_FAILED},
    sui::{
        args::NewPackageArgs,
        project::{CliContext, resolve_context},
        runners::{run_build, run_new_package, run_test},
    },
};
use codex_exec_server::LOCAL_FS;
use codex_git_utils::resolve_root_git_project_for_trust;
use peregrine_config::LoaderOverrides;
use peregrine_move_model::MovePackageModel;
use peregrine_scanner::{
    core::{PackageScanner, ScanInput, ScannerDiagnosticSeverity, ScannerOutput, SourceMode},
    sui::tests::{TestsScanReport, TestsScanner},
};
use peregrine_static_analysis::{MovePackage, discover_move_project_fast};
use peregrine_types::config_types::TrustLevel;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Clone, Debug)]
pub(crate) enum PackageInspection {
    Valid { context: CliContext },
    Invalid { root: PathBuf, message: String },
}

pub(crate) fn inspect_package_directory(root: impl AsRef<Path>) -> PackageInspection {
    let input = root.as_ref();
    let root = match input.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            return PackageInspection::Invalid {
                root: input.to_path_buf(),
                message: format!("Could not read {}: {error}", input.display()),
            };
        }
    };

    if !root.is_dir() {
        return PackageInspection::Invalid {
            root,
            message: "Selected path is not a directory.".to_string(),
        };
    }

    if !root.join("Move.toml").is_file() {
        return PackageInspection::Invalid {
            root,
            message: "Selected directory does not contain a Move.toml manifest.".to_string(),
        };
    }

    match resolve_context(&root, ".") {
        Ok(context) => PackageInspection::Valid { context },
        Err(error) => PackageInspection::Invalid {
            root,
            message: error.message,
        },
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WorkbenchTrustResolution {
    pub cwd: PathBuf,
    pub trust_target: PathBuf,
    pub peregrine_home: PathBuf,
    pub trust_level: Option<TrustLevel>,
}

impl WorkbenchTrustResolution {
    pub(crate) fn is_trusted(&self) -> bool {
        matches!(self.trust_level, Some(TrustLevel::Trusted))
    }

    pub(crate) fn is_untrusted(&self) -> bool {
        matches!(self.trust_level, Some(TrustLevel::Untrusted))
    }
}

pub(crate) fn resolve_trust_for_directory(
    root: impl AsRef<Path>,
) -> std::io::Result<WorkbenchTrustResolution> {
    let root = root.as_ref().to_path_buf();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(std::io::Error::other)?;
    runtime.block_on(resolve_trust_for_directory_async(root))
}

async fn resolve_trust_for_directory_async(
    root: PathBuf,
) -> std::io::Result<WorkbenchTrustResolution> {
    let config = peregrine_core::config::ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::default())
        .fallback_cwd(Some(root))
        .build()
        .await?;
    let trust_target = resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &config.cwd)
        .await
        .map(Into::into)
        .unwrap_or_else(|| config.cwd.to_path_buf());

    Ok(WorkbenchTrustResolution {
        cwd: config.cwd.to_path_buf(),
        trust_target,
        peregrine_home: config.peregrine_home.to_path_buf(),
        trust_level: config.active_project.trust_level,
    })
}

pub(crate) fn persist_trust_for_resolution(
    resolution: &WorkbenchTrustResolution,
) -> Result<(), String> {
    persist_trusted_project(&resolution.peregrine_home, &resolution.trust_target)
}

pub(crate) fn persist_trusted_project(
    peregrine_home: &Path,
    project_root: &Path,
) -> Result<(), String> {
    peregrine_core::config::set_project_trust_level(
        peregrine_home,
        project_root,
        TrustLevel::Trusted,
    )
    .map_err(|error| {
        format!(
            "failed to persist trusted project {} in {}: {error}",
            project_root.display(),
            peregrine_home.display()
        )
    })
}

pub(crate) fn persist_created_package_trust(project_root: &Path) -> Result<(), String> {
    let peregrine_home = peregrine_utils_home_dir::find_peregrine_home()
        .map_err(|error| format!("Could not resolve Peregrine home: {error}"))?;
    persist_trusted_project(peregrine_home.as_path(), project_root)
}

#[derive(Clone, Debug)]
pub(crate) struct PackageCreateReport {
    pub step: CliStep,
    pub package_root: PathBuf,
}

pub(crate) fn create_child_move_package(parent: &Path, package_name: &str) -> PackageCreateReport {
    let step = run_new_package(
        parent,
        &NewPackageArgs {
            package_name: package_name.to_string(),
        },
    );
    let package_root = parent.join(package_name.trim());

    PackageCreateReport { step, package_root }
}

#[derive(Clone, Debug)]
pub(crate) struct PackageLoadReport {
    #[allow(dead_code)]
    pub package_root: PathBuf,
    pub build: CliStep,
    pub test: CliStep,
    pub scanners: PackageScannerReport,
}

#[derive(Clone, Debug)]
pub(crate) struct PackageScannerReport {
    pub compiler_unit_tests: ScannerResult,
    pub compiler_movy_invariant_tests: ScannerResult,
    pub compiler_fuzz_tests: ScannerResult,
    pub compiler_formal_verification: ScannerResult,
    pub heuristic_unit_tests: ScannerResult,
    pub heuristic_movy_invariant_tests: ScannerResult,
    pub heuristic_fuzz_tests: ScannerResult,
    pub heuristic_formal_verification: ScannerResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ScannerResult {
    Found { count: usize },
    NotFound,
    Failed { message: String },
    Unavailable { reason: String },
}

pub(crate) fn load_package_after_trust(context: CliContext) -> PackageLoadReport {
    let build = run_build(&context);
    let test = run_test(&context);
    let scanners = run_test_scanners(&context, build.status == CliStatus::Passed);

    PackageLoadReport {
        package_root: context.package_root,
        build,
        test,
        scanners,
    }
}

fn run_test_scanners(
    context: &CliContext,
    compiler_output_available: bool,
) -> PackageScannerReport {
    let heuristic = scan_tests_with_mode(context, SourceMode::SourceOnly, true);
    let heuristic_unit_tests = unit_test_result(&heuristic);
    let heuristic_movy_invariant_tests = movy_invariant_result(&heuristic);
    let heuristic_fuzz_tests = fuzz_test_result(&heuristic);
    let heuristic_formal_verification = formal_verification_result(&heuristic);

    if !compiler_output_available {
        let reason = "build/compiler output is unavailable".to_string();
        return PackageScannerReport {
            compiler_unit_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_movy_invariant_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_fuzz_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_formal_verification: ScannerResult::Unavailable { reason },
            heuristic_unit_tests,
            heuristic_movy_invariant_tests,
            heuristic_fuzz_tests,
            heuristic_formal_verification,
        };
    }

    let compiler = scan_tests_with_mode(context, SourceMode::BestAvailable, false);

    PackageScannerReport {
        compiler_unit_tests: unit_test_result(&compiler),
        compiler_movy_invariant_tests: movy_invariant_result(&compiler),
        compiler_fuzz_tests: fuzz_test_result(&compiler),
        compiler_formal_verification: formal_verification_result(&compiler),
        heuristic_unit_tests,
        heuristic_movy_invariant_tests,
        heuristic_fuzz_tests,
        heuristic_formal_verification,
    }
}

fn unit_test_result(scan: &Result<TestsScanReport, String>) -> ScannerResult {
    match scan {
        Ok(report) if report.has_unit_tests => ScannerResult::Found {
            count: report.unit_test_count,
        },
        Ok(_) => ScannerResult::NotFound,
        Err(message) => ScannerResult::Failed {
            message: message.clone(),
        },
    }
}

fn movy_invariant_result(scan: &Result<TestsScanReport, String>) -> ScannerResult {
    match scan {
        Ok(report) if report.has_movy_invariant_tests => ScannerResult::Found {
            count: report.movy_invariant_test_count,
        },
        Ok(_) => ScannerResult::NotFound,
        Err(message) => ScannerResult::Failed {
            message: message.clone(),
        },
    }
}

fn fuzz_test_result(scan: &Result<TestsScanReport, String>) -> ScannerResult {
    match scan {
        Ok(report) => {
            let count = report
                .unit_tests
                .iter()
                .filter(|finding| finding.is_random_test)
                .count();
            if count > 0 {
                ScannerResult::Found { count }
            } else {
                ScannerResult::NotFound
            }
        }
        Err(message) => ScannerResult::Failed {
            message: message.clone(),
        },
    }
}

fn formal_verification_result(scan: &Result<TestsScanReport, String>) -> ScannerResult {
    match scan {
        Ok(report) if report.has_formal_prover_specs => ScannerResult::Found {
            count: report.formal_prover_spec_count,
        },
        Ok(_) => ScannerResult::NotFound,
        Err(message) => ScannerResult::Failed {
            message: message.clone(),
        },
    }
}

fn scan_tests_with_mode(
    context: &CliContext,
    source_mode: SourceMode,
    force_source_model: bool,
) -> Result<TestsScanReport, String> {
    let mut model = selected_package_model(context)?;
    if force_source_model {
        model.modules.clear();
    }
    let build_root = context.package_root.join("build").join(&model.name);
    let input = ScanInput {
        package_model: &model,
        package_root: Some(context.package_root.clone()),
        build_root: Some(build_root),
        source_mode,
    };

    match TestsScanner.scan(&input) {
        ScannerOutput::Tests(report) => {
            if let Some(error) = report
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.severity == ScannerDiagnosticSeverity::Error)
            {
                Err(error.message.clone())
            } else {
                Ok(report)
            }
        }
        ScannerOutput::Objects(_) => {
            Err("tests scanner returned an object scan report".to_string())
        }
    }
}

fn selected_package_model(context: &CliContext) -> Result<MovePackageModel, String> {
    let project = discover_move_project_fast(&context.project_root);
    let package = project
        .packages
        .into_iter()
        .find(|package| package_path_matches(&package.path, &context.package_path))
        .ok_or_else(|| {
            format!(
                "Could not discover package `{}` under {}.",
                context.package_path,
                context.project_root.display()
            )
        })?;

    Ok(move_package_to_model(package))
}

fn move_package_to_model(package: MovePackage) -> MovePackageModel {
    MovePackageModel {
        name: package.name,
        path: package.path,
        manifest_path: package.manifest_path,
        has_source_files: package.has_source_files,
        has_source_modules: package.has_source_modules,
        source_file_count: package.source_file_count,
        modules: package.modules,
    }
}

fn package_path_matches(discovered_path: &str, requested_path: &str) -> bool {
    normalize_package_path(discovered_path) == normalize_package_path(requested_path)
}

fn normalize_package_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "." {
        ".".to_string()
    } else {
        Path::new(trimmed)
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                std::path::Component::CurDir => None,
                other => Some(other.as_os_str().to_string_lossy().into_owned()),
            })
            .collect::<Vec<_>>()
            .join("/")
    }
}

pub(crate) fn skipped_trust_step(name: &str, message: impl Into<String>) -> CliStep {
    CliStep {
        name: name.to_string(),
        status: CliStatus::Skipped,
        duration_ms: 0,
        exit_code: 0,
        command: None,
        diagnostics: vec![CliDiagnostic {
            severity: CliDiagnosticSeverity::Info,
            source: "trust".to_string(),
            code: Some("TrustDenied".to_string()),
            message: message.into(),
            file: None,
            span: None,
        }],
        metadata: Default::default(),
        stdout: String::new(),
        stderr: String::new(),
        details: json!({ "trust": "denied" }),
    }
}

pub(crate) fn failed_startup_step(name: &str, message: impl Into<String>) -> CliStep {
    CliStep::failed(
        name.to_string(),
        Instant::now(),
        CliDiagnostic {
            severity: CliDiagnosticSeverity::Error,
            source: "startup".to_string(),
            code: Some("StartupFailed".to_string()),
            message: message.into(),
            file: None,
            span: None,
        },
    )
}

pub(crate) fn failed_create_report(
    parent: &Path,
    package_name: &str,
    message: String,
) -> PackageCreateReport {
    PackageCreateReport {
        step: failed_startup_step("new-package", message),
        package_root: parent.join(package_name.trim()),
    }
}

pub(crate) fn trust_denied_load_report(
    package_root: PathBuf,
    message: String,
) -> PackageLoadReport {
    let build = skipped_trust_step("build", message.clone());
    let test = skipped_trust_step("test", message);
    let reason = "project trust was denied".to_string();

    PackageLoadReport {
        package_root,
        build,
        test,
        scanners: PackageScannerReport {
            compiler_unit_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_movy_invariant_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_fuzz_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_formal_verification: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_unit_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_movy_invariant_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_fuzz_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_formal_verification: ScannerResult::Unavailable { reason },
        },
    }
}

pub(crate) fn workflow_failed_status(step: &CliStep) -> bool {
    step.status == CliStatus::Failed || step.exit_code == EXIT_WORKFLOW_FAILED
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn inspect_directory_without_manifest_is_invalid() {
        let temp = tempfile::tempdir().expect("temp dir");

        let inspection = inspect_package_directory(temp.path());

        assert!(matches!(inspection, PackageInspection::Invalid { .. }));
    }

    #[test]
    fn inspect_directory_with_manifest_is_valid() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");

        let inspection = inspect_package_directory(temp.path());

        assert!(matches!(inspection, PackageInspection::Valid { .. }));
    }

    #[test]
    fn scanner_reports_compiler_unavailable_when_build_failed() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("tests")).expect("tests");
        fs::write(
            temp.path().join("tests/test_demo.move"),
            r#"
module demo::test_demo;

#[test]
fun test_flow() {}
"#,
        )
        .expect("test source");
        let PackageInspection::Valid { context } = inspect_package_directory(temp.path()) else {
            panic!("expected valid package");
        };

        let scanners = run_test_scanners(&context, false);

        assert!(matches!(
            scanners.compiler_unit_tests,
            ScannerResult::Unavailable { .. }
        ));
        assert!(matches!(
            scanners.heuristic_unit_tests,
            ScannerResult::Found { count: 1 }
        ));
    }

    #[test]
    fn scanner_reports_not_found_distinct_from_unavailable() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("sources/demo.move"),
            "module demo::demo; public fun ping() {}",
        )
        .expect("source");
        let PackageInspection::Valid { context } = inspect_package_directory(temp.path()) else {
            panic!("expected valid package");
        };

        let scanners = run_test_scanners(&context, true);

        assert_eq!(scanners.compiler_unit_tests, ScannerResult::NotFound);
        assert_eq!(scanners.heuristic_unit_tests, ScannerResult::NotFound);
    }
}
