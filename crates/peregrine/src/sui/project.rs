use crate::{
    output::{CliDiagnostic, CliDiagnosticSeverity},
    sui::args::VerifyArgs,
};
use peregrine_static_analysis::{discover_move_project_fast, MoveModule};
use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct CliContext {
    pub project_root: PathBuf,
    pub package_path: String,
    pub package_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalTarget {
    pub file_path: String,
    pub module_name: String,
}

pub fn resolve_workspace_root(project_root: impl AsRef<Path>) -> Result<PathBuf, CliDiagnostic> {
    let project_input = project_root.as_ref();
    let project_root = project_input.canonicalize().map_err(|error| {
        CliDiagnostic::error(
            "project",
            format!(
                "Could not read project root {}: {error}",
                project_input.display()
            ),
        )
    })?;

    if !project_root.is_dir() {
        return Err(CliDiagnostic::error(
            "project",
            format!(
                "Project root {} is not a directory.",
                project_root.display()
            ),
        ));
    }

    Ok(project_root)
}

pub fn resolve_context(
    project_root: impl AsRef<Path>,
    package_path: &str,
) -> Result<CliContext, CliDiagnostic> {
    let project_root = resolve_workspace_root(project_root)?;
    let package_root = resolve_package_root(&project_root, package_path)?;
    let package_path = relative_package_path(&project_root, &package_root);

    Ok(CliContext {
        project_root,
        package_path,
        package_root,
    })
}

pub fn formal_targets(
    context: &CliContext,
    args: &VerifyArgs,
) -> Result<Vec<FormalTarget>, CliDiagnostic> {
    let project = discover_move_project_fast(&context.project_root);
    let Some(package) = project
        .packages
        .iter()
        .find(|package| normalize_path_label(Path::new(&package.path)) == context.package_path)
    else {
        return Err(CliDiagnostic::error(
            "verify",
            format!(
                "Could not discover package `{}` under {}.",
                context.package_path,
                context.project_root.display()
            ),
        ));
    };
    let requested_modules = args
        .modules
        .iter()
        .map(|module| module.trim())
        .filter(|module| !module.is_empty())
        .collect::<Vec<_>>();
    let requested_file = args
        .file
        .as_deref()
        .map(normalize_requested_file)
        .filter(|file| !file.is_empty());

    let targets = package
        .modules
        .iter()
        .filter(|module| {
            requested_file.as_deref().map_or(true, |file| {
                normalize_path_label(Path::new(&module.file_path)) == file
            })
        })
        .filter(|module| {
            requested_modules.is_empty()
                || requested_modules
                    .iter()
                    .any(|requested| module_matches(requested, module))
        })
        .map(|module| FormalTarget {
            file_path: module.file_path.clone(),
            module_name: module.name.clone(),
        })
        .collect::<Vec<_>>();

    if targets.is_empty() {
        return Err(CliDiagnostic {
            severity: CliDiagnosticSeverity::Error,
            source: "verify".to_string(),
            code: Some("NoFormalTargets".to_string()),
            message: "No Move modules matched the requested formal verification target."
                .to_string(),
            file: requested_file,
            span: None,
        });
    }

    Ok(targets)
}

pub fn resolve_output_path(workspace_root: &Path, output: Option<&Path>) -> PathBuf {
    match output {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => workspace_root.join(path),
        None => workspace_root.to_path_buf(),
    }
}

fn resolve_package_root(project_root: &Path, package_path: &str) -> Result<PathBuf, CliDiagnostic> {
    let package_path = package_path.trim();
    let package_root = if package_path.is_empty() || package_path == "." {
        project_root.to_path_buf()
    } else {
        let path = Path::new(package_path);

        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(CliDiagnostic::error(
                "project",
                "Package path cannot contain `..` components.",
            ));
        }

        if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_root.join(path)
        }
    };
    let package_root = package_root.canonicalize().map_err(|error| {
        CliDiagnostic::error(
            "project",
            format!("Could not read package path {}: {error}", package_path),
        )
    })?;

    if !package_root.starts_with(project_root) {
        return Err(CliDiagnostic::error(
            "project",
            "Package path must be inside the project root.",
        ));
    }

    if !package_root.join("Move.toml").is_file() {
        return Err(CliDiagnostic::error(
            "project",
            format!(
                "Package path {} does not contain a Move.toml file.",
                package_root.display()
            ),
        ));
    }

    Ok(package_root)
}

fn relative_package_path(project_root: &Path, package_root: &Path) -> String {
    let relative = package_root
        .strip_prefix(project_root)
        .unwrap_or(package_root);

    if relative.as_os_str().is_empty() {
        return ".".to_string();
    }

    normalize_path_label(relative)
}

fn normalize_requested_file(file: &str) -> String {
    normalize_path_label(Path::new(file.trim()))
}

fn normalize_path_label(path: impl AsRef<Path>) -> String {
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

fn module_matches(requested: &str, module: &MoveModule) -> bool {
    if requested == module.name {
        return true;
    }

    module
        .address
        .as_deref()
        .is_some_and(|address| requested == format!("{address}::{}", module.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolves_package_inside_project_root() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("pkg/sources")).expect("sources");
        fs::write(
            temp.path().join("pkg/Move.toml"),
            "[package]\nname = \"pkg\"\n",
        )
        .expect("manifest");

        let context = resolve_context(temp.path(), "pkg").expect("context");

        assert_eq!(context.package_path, "pkg");
        assert_eq!(
            context.package_root,
            temp.path().join("pkg").canonicalize().unwrap()
        );
    }

    #[test]
    fn rejects_package_path_escape() {
        let temp = tempdir().expect("tempdir");
        let error = resolve_context(temp.path(), "../outside").expect_err("escape");

        assert_eq!(error.severity, CliDiagnosticSeverity::Error);
    }

    #[test]
    fn discovers_formal_targets_for_root_package() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::write(
            temp.path().join("sources/m.move"),
            "module demo::m { public fun ping() {} }",
        )
        .expect("source");
        let context = resolve_context(temp.path(), ".").expect("context");
        let args = VerifyArgs {
            modules: Vec::new(),
            file: None,
            timeout_seconds: 45,
            trace: false,
            keep_temp: false,
        };

        let targets = formal_targets(&context, &args).expect("targets");

        assert_eq!(
            targets,
            vec![FormalTarget {
                file_path: "sources/m.move".to_string(),
                module_name: "m".to_string(),
            }]
        );
    }

    #[test]
    fn relative_output_path_is_workspace_scoped() {
        let root = Path::new("/workspace");

        assert_eq!(
            resolve_output_path(root, Some(Path::new("imports/pkg"))),
            PathBuf::from("/workspace/imports/pkg")
        );
    }
}
