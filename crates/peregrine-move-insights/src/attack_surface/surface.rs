use peregrine_move_model::{MoveModule, MovePackageModel};
use std::path::PathBuf;

use crate::scanner_report::{MovePackageScannerReport, package_scanner_report_for_package};

use super::{
    admin::admin_control_findings,
    calls::{external_call_findings, public_package_relationships},
    from_scanner_report::{
        capability_findings_from_object_scan, object_lifecycle_maps_from_object_scan,
        object_ownership_findings_from_object_scan, ownership_count,
    },
    types::MovePackageSurface,
};

pub fn package_surface(modules: &[MoveModule]) -> MovePackageSurface {
    let model = MovePackageModel {
        name: "package".to_string(),
        path: String::new(),
        manifest_path: String::new(),
        has_source_files: !modules.is_empty(),
        has_source_modules: !modules.is_empty(),
        source_file_count: modules.len(),
        modules: modules.to_vec(),
    };

    package_surface_for_package(&model, None, None)
}

pub fn package_surface_for_package(
    model: &MovePackageModel,
    package_root: Option<PathBuf>,
    build_root: Option<PathBuf>,
) -> MovePackageSurface {
    let scanner_report = package_scanner_report_for_package(model, package_root, build_root);
    package_surface_from_scanner_report(model, &scanner_report)
}

pub fn package_surface_from_scanner_report(
    model: &MovePackageModel,
    scanner_report: &MovePackageScannerReport,
) -> MovePackageSurface {
    let modules = &model.modules;
    let entry_function_count = modules
        .iter()
        .flat_map(|module| module.functions.iter())
        .filter(|function| function.is_transaction_callable)
        .count();
    let object_scan = &scanner_report.objects;
    let capability_findings = capability_findings_from_object_scan(object_scan);
    let mut capability_structs = capability_findings
        .iter()
        .filter(|finding| finding.confidence != "low")
        .map(|finding| finding.qualified_name.clone())
        .collect::<Vec<_>>();
    let object_ownership_findings = object_ownership_findings_from_object_scan(object_scan);
    let object_lifecycle_maps = object_lifecycle_maps_from_object_scan(object_scan, modules);
    let admin_control_findings = admin_control_findings(modules, &capability_structs);
    let external_call_findings = external_call_findings(modules);
    let public_package_relationships = public_package_relationships(modules);
    let mut shared_object_structs = object_scan.shared_object_structs.clone();

    capability_structs.sort();
    capability_structs.dedup();
    shared_object_structs.sort();
    shared_object_structs.dedup();

    MovePackageSurface {
        entry_function_count,
        capability_count: capability_structs.len(),
        shared_object_count: ownership_count(&object_ownership_findings, "shared")
            .max(shared_object_structs.len()),
        address_owned_object_count: ownership_count(&object_ownership_findings, "addressOwned"),
        immutable_object_count: ownership_count(&object_ownership_findings, "immutable"),
        wrapped_object_count: ownership_count(&object_ownership_findings, "wrapped"),
        party_object_count: ownership_count(&object_ownership_findings, "party"),
        admin_control_count: admin_control_findings.len(),
        external_call_count: external_call_findings.len(),
        public_package_relationship_count: public_package_relationships.len(),
        capability_structs,
        capability_findings,
        shared_object_structs,
        object_lifecycle_maps,
        object_ownership_findings,
        admin_control_findings,
        external_call_findings,
        public_package_relationships,
    }
}
