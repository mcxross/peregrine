use super::McpToolClient;
use peregrine_mcp_protocol::{
    MAX_PAGE_SIZE, ModuleEntry, ModulesArgs, ModulesPage, MoveSourceSummary, PackageArgs,
    PackageSummary, SignatureEntry, SignaturesArgs, SignaturesPage, tool_name,
};
use std::path::Path;

pub(crate) fn fetch_modules(
    project_root: &Path,
    package_path: &str,
    modules: Vec<String>,
    file: Option<String>,
) -> Result<(PackageSummary, MoveSourceSummary, Vec<ModuleEntry>), String> {
    let mut cursor = None;
    let mut entries = Vec::new();

    loop {
        let request = ModulesArgs {
            package: PackageArgs {
                project_root: Some(project_root.display().to_string()),
                package_path: Some(package_path.to_string()),
            },
            modules: modules.clone(),
            file: file.clone(),
            cursor,
            limit: Some(MAX_PAGE_SIZE),
        };
        let page = McpToolClient::call_blocking::<_, ModulesPage>(
            project_root,
            tool_name::MODULES,
            &request,
        )?;
        entries.extend(page.data);

        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok((page.package, page.source, entries)),
        }
    }
}

pub(crate) fn fetch_signatures(
    project_root: &Path,
    package_path: &str,
    modules: Vec<String>,
    file: Option<String>,
) -> Result<(PackageSummary, Vec<SignatureEntry>), String> {
    let mut cursor = None;
    let mut signatures = Vec::new();

    loop {
        let request = SignaturesArgs {
            package: PackageArgs {
                project_root: Some(project_root.display().to_string()),
                package_path: Some(package_path.to_string()),
            },
            modules: modules.clone(),
            file: file.clone(),
            cursor,
            limit: Some(MAX_PAGE_SIZE),
        };
        let page = McpToolClient::call_blocking::<_, SignaturesPage>(
            project_root,
            tool_name::SIGNATURES,
            &request,
        )?;
        signatures.extend(page.data);

        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok((page.package, signatures)),
        }
    }
}
