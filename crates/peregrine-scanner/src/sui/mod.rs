pub mod facts;
pub mod objects;

use crate::{
    core::{PackageScanner, ScanInput, ScanReport, ScannerOutput},
    sui::objects::ObjectScanner,
};

pub fn scan_package(input: ScanInput<'_>) -> ScanReport {
    let package_id = input.package_model.name.clone();
    let scanners = vec![ObjectScanner.scan(&input)];
    let diagnostics = scanners
        .iter()
        .flat_map(|output| match output {
            ScannerOutput::Objects(objects) => objects.diagnostics.clone(),
        })
        .collect();

    ScanReport {
        package_id,
        scanners,
        diagnostics,
    }
}
