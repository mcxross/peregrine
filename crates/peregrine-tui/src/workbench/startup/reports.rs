use crate::output::CliStep;
use crate::sui::package_loader::{
    PackageLoadReport, PackageScannerReport, ScannerResult, failed_startup_step,
};
use std::path::PathBuf;

pub(crate) fn startup_failure_load_report(
    package_root: PathBuf,
    message: String,
) -> PackageLoadReport {
    let reason = message.clone();
    PackageLoadReport {
        package_root,
        build: failed_startup_step("build", message),
        test: CliStep::skipped("test", "package loading could not start"),
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
