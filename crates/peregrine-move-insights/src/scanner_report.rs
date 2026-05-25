use peregrine_move_model::{MoveModule, MovePackageModel};
use peregrine_scanner::{
    core::{ScanInput, ScannerDiagnostic, ScannerOutput, SourceMode},
    sui::{objects::ObjectScanReport, scan_package, tests::TestsScanReport},
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageScannerReport {
    pub package_id: String,
    pub objects: ObjectScanReport,
    pub tests: TestsScanReport,
    pub diagnostics: Vec<ScannerDiagnostic>,
}

pub fn package_scanner_report(modules: &[MoveModule]) -> MovePackageScannerReport {
    let model = MovePackageModel {
        name: "package".to_string(),
        path: String::new(),
        manifest_path: String::new(),
        has_source_files: !modules.is_empty(),
        has_source_modules: !modules.is_empty(),
        source_file_count: modules.len(),
        modules: modules.to_vec(),
    };

    package_scanner_report_for_package(&model, None, None)
}

pub fn package_scanner_report_for_package(
    model: &MovePackageModel,
    package_root: Option<PathBuf>,
    build_root: Option<PathBuf>,
) -> MovePackageScannerReport {
    let report = scan_package(ScanInput {
        package_model: model,
        package_root,
        build_root,
        source_mode: SourceMode::BestAvailable,
    });
    let package_id = report.package_id;
    let diagnostics = report.diagnostics;
    let mut objects = None;
    let mut tests = None;

    for output in report.scanners {
        match output {
            ScannerOutput::Objects(object_report) => objects = Some(object_report),
            ScannerOutput::Tests(tests_report) => tests = Some(tests_report),
        }
    }

    MovePackageScannerReport {
        package_id,
        objects: objects.unwrap_or_else(empty_object_scan_report),
        tests: tests.unwrap_or_else(empty_tests_scan_report),
        diagnostics,
    }
}

fn empty_object_scan_report() -> ObjectScanReport {
    ObjectScanReport {
        capability_findings: Vec::new(),
        ownership_findings: Vec::new(),
        lifecycle_maps: Vec::new(),
        shared_object_structs: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn empty_tests_scan_report() -> TestsScanReport {
    TestsScanReport {
        has_unit_tests: false,
        has_movy_invariant_tests: false,
        has_formal_prover_specs: false,
        unit_test_count: 0,
        movy_invariant_test_count: 0,
        formal_prover_spec_count: 0,
        unit_tests: Vec::new(),
        movy_invariant_tests: Vec::new(),
        formal_prover_specs: Vec::new(),
        diagnostics: Vec::new(),
    }
}
