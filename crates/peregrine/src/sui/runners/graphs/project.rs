use crate::{
    output::CliDiagnostic,
    sui::project::{require_package_source_modules, CliContext},
};
use peregrine_static_analysis::{discover_move_project_fast, MovePackage};
use std::{
    ffi::OsStr,
    path::{Component, Path},
};

pub fn selected_package(context: &CliContext, source: &str) -> Result<MovePackage, CliDiagnostic> {
    let project = discover_move_project_fast(&context.project_root);

    project
        .packages
        .into_iter()
        .find(|package| normalize_path_label(Path::new(&package.path)) == context.package_path)
        .ok_or_else(|| {
            CliDiagnostic::error(
                source,
                format!(
                    "Could not discover package `{}` under {}.",
                    context.package_path,
                    context.project_root.display()
                ),
            )
        })
}

pub fn selected_source_package(
    context: &CliContext,
    source: &str,
) -> Result<MovePackage, CliDiagnostic> {
    let package = selected_package(context, source)?;

    require_package_source_modules(source, &package)?;

    Ok(package)
}

pub fn module_matches(requested: &str, address: Option<&str>, module_name: &str) -> bool {
    if requested == module_name {
        return true;
    }

    address.is_some_and(|address| requested == format!("{address}::{module_name}"))
}

pub fn normalize_path_label(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();

    if path.as_os_str().is_empty() || path == Path::new(".") {
        return ".".to_string();
    }

    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            Component::CurDir => None,
            _ => Some(component.as_os_str()),
        })
        .map(OsStr::to_string_lossy)
        .collect::<Vec<_>>()
        .join("/")
}
