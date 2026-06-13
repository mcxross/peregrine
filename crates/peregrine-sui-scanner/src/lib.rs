mod analysis;
pub mod core;
pub mod facts;
pub mod objects;
pub mod tests;

pub use analysis::SuiScanner;
pub use core::{
    EvidenceSource, ScanInput, ScanReport, ScannerConfidence, ScannerDiagnostic,
    ScannerDiagnosticSeverity, ScannerOutput, SourceMode,
};

use core::PackageScanner;
use objects::ObjectScanner;
use tests::TestsScanner;

pub fn scan_package(input: ScanInput<'_>) -> ScanReport {
    let package_id = input.package_model.name.clone();
    let scanners = vec![ObjectScanner.scan(&input), TestsScanner.scan(&input)];
    let diagnostics = scanners
        .iter()
        .flat_map(|output| match output {
            ScannerOutput::Objects(objects) => objects.diagnostics.clone(),
            ScannerOutput::Tests(tests) => tests.diagnostics.clone(),
        })
        .collect();

    ScanReport {
        package_id,
        scanners,
        diagnostics,
    }
}
