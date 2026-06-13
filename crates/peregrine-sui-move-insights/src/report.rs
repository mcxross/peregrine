use peregrine_sui_move_model::{MoveModule, MovePackageModel};
use serde::Serialize;
use std::path::PathBuf;

use crate::{
    attack_surface::{MovePackageSurface, package_surface_from_scanner_report},
    scanner_report::{
        MovePackageScannerReport, package_scanner_report, package_scanner_report_for_package,
    },
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageInsightsReport {
    pub scanner_report: MovePackageScannerReport,
    pub attack_surface: MovePackageSurface,
}

pub fn package_insights_report(modules: &[MoveModule]) -> MovePackageInsightsReport {
    let scanner_report = package_scanner_report(modules);
    let model = MovePackageModel {
        name: scanner_report.package_id.clone(),
        path: String::new(),
        manifest_path: String::new(),
        has_source_files: !modules.is_empty(),
        has_source_modules: !modules.is_empty(),
        source_file_count: modules.len(),
        modules: modules.to_vec(),
    };

    package_insights_report_from_scanner_report(&model, scanner_report)
}

pub fn package_insights_report_for_package(
    model: &MovePackageModel,
    package_root: Option<PathBuf>,
    build_root: Option<PathBuf>,
) -> MovePackageInsightsReport {
    let scanner_report = package_scanner_report_for_package(model, package_root, build_root);
    package_insights_report_from_scanner_report(model, scanner_report)
}

pub fn package_insights_report_from_scanner_report(
    model: &MovePackageModel,
    scanner_report: MovePackageScannerReport,
) -> MovePackageInsightsReport {
    let attack_surface = package_surface_from_scanner_report(model, &scanner_report);

    MovePackageInsightsReport {
        scanner_report,
        attack_surface,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use peregrine_sui_move_model::{MovePackageModel, parse_module_declarations};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn insights_report_contains_scanner_objects_tests_and_attack_surface() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        let source = r#"
module demo::vault;

public struct AdminCap has key, store { id: UID }
public struct Vault has key, store { id: UID }

public entry fun init(ctx: &mut TxContext) {
    transfer::transfer(AdminCap { id: object::new(ctx) }, tx_context::sender(ctx));
    transfer::share_object(Vault { id: object::new(ctx) });
}
"#;
        let source_path = temp.path().join("sources/vault.move");
        fs::create_dir_all(source_path.parent().expect("sources parent")).expect("mkdir sources");
        fs::write(&source_path, source).expect("source");
        let test_path = temp.path().join("tests/vault_tests.move");
        fs::create_dir_all(test_path.parent().expect("tests parent")).expect("mkdir tests");
        fs::write(
            &test_path,
            r#"
#[test_only]
module demo::vault_tests;

#[test]
fun test_init() {}
"#,
        )
        .expect("tests");
        let modules = parse_module_declarations(source, temp.path(), &source_path);
        let model = MovePackageModel {
            name: "demo".to_string(),
            path: String::new(),
            manifest_path: "Move.toml".to_string(),
            has_source_files: true,
            has_source_modules: true,
            source_file_count: 1,
            modules,
        };

        let report = package_insights_report_for_package(
            &model,
            Some(temp.path().to_path_buf()),
            Some(temp.path().join("build/demo")),
        );

        assert!(report.scanner_report.objects.capability_findings.len() >= 1);
        assert!(report.scanner_report.tests.has_unit_tests);
        assert_eq!(report.scanner_report.tests.unit_test_count, 1);
        assert!(report.attack_surface.capability_count >= 1);
        assert!(report.attack_surface.shared_object_count >= 1);
    }
}
