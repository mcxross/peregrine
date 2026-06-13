use crate::{core::stable_id, model::DependencyRecord};

pub fn immediate_dependency_record(
    package_id: &str,
    source_package_alias: &str,
    source_module: &str,
    target_package_alias: &str,
    target_module: &str,
) -> DependencyRecord {
    DependencyRecord {
        id: stable_id(
            "dependency",
            [
                package_id,
                source_package_alias,
                source_module,
                target_package_alias,
                target_module,
            ],
        ),
        package_id: package_id.to_string(),
        source_package_alias: source_package_alias.to_string(),
        source_module: source_module.to_string(),
        target_package_alias: target_package_alias.to_string(),
        target_module: target_module.to_string(),
        dependency_kind: "ImmediateModuleDependency".to_string(),
        metadata_json: None,
    }
}

pub fn dependency_target(package_alias: &str, module_name: &str) -> String {
    format!("{package_alias}::{module_name}")
}
