use crate::core::{MaterializedStatus, ModuleInfo, SourceSpan, SummaryArtifact, logical_id};

pub fn module_info(
    package_id: &str,
    package_alias: &str,
    module_name: &str,
    summary_artifact: &SummaryArtifact,
    immediate_dependencies: Vec<String>,
    docs: Option<String>,
    attributes: Vec<String>,
) -> ModuleInfo {
    let module_id = logical_id("module", [package_id, package_alias, module_name]);
    ModuleInfo {
        id: module_id,
        package_id: package_id.to_string(),
        summary_artifact_id: Some(summary_artifact.id.clone()),
        file_id: None,
        address: package_alias.to_string(),
        name: module_name.to_string(),
        full_name: format!("{package_alias}::{module_name}"),
        immediate_dependencies,
        docs,
        attributes,
        source_span: SourceSpan::summary_artifact(summary_artifact.id.clone()),
    }
}

pub fn materialization_rank(status: &MaterializedStatus) -> u8 {
    match status {
        MaterializedStatus::PointerOnly => 0,
        MaterializedStatus::RootCard => 1,
        MaterializedStatus::DirectDependencyCard => 2,
        MaterializedStatus::ExpandedModule => 3,
        MaterializedStatus::ExpandedSymbol => 4,
    }
}
