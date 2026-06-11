use crate::{
    output::{CliDiagnostic, CliDiagnosticSeverity},
    session::fetch_modules,
    sui::args::{BytecodeArgs, VerifyArgs},
};
use peregrine_mcp_protocol::{MoveSourceSummary, PackageSummary};
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytecodeTarget {
    pub file_path: String,
    pub module_name: String,
    pub source_path: PathBuf,
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
    let modules = args
        .modules
        .iter()
        .map(|module| module.trim())
        .filter(|module| !module.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let file = args
        .file
        .as_deref()
        .map(normalize_requested_file)
        .filter(|file| !file.is_empty());
    let (_, _, modules) = fetch_modules(
        &context.project_root,
        &context.package_path,
        modules,
        file.clone(),
    )
    .map_err(|error| CliDiagnostic::error("mcp:peregrine", error))?;
    let targets = modules
        .into_iter()
        .map(|module| FormalTarget {
            file_path: module.file_path,
            module_name: module.module_name,
        })
        .collect::<Vec<_>>();

    if targets.is_empty() {
        return Err(CliDiagnostic {
            severity: CliDiagnosticSeverity::Error,
            source: "verify".to_string(),
            code: Some("NoFormalTargets".to_string()),
            message: "No Move modules matched the requested formal verification target."
                .to_string(),
            file,
            span: None,
        });
    }

    Ok(targets)
}

pub fn bytecode_target(
    context: &CliContext,
    args: &BytecodeArgs,
) -> Result<BytecodeTarget, CliDiagnostic> {
    let requested_module = args
        .module
        .as_deref()
        .map(str::trim)
        .filter(|module| !module.is_empty());
    let requested_file = args
        .file
        .as_deref()
        .map(normalize_requested_file)
        .filter(|file| !file.is_empty());
    let targets = bytecode_targets(context, requested_module, requested_file.as_deref())?;

    match targets.as_slice() {
        [] => Err(no_bytecode_target(requested_file)),
        [target] => Ok(target.clone()),
        _ => Err(CliDiagnostic {
            severity: CliDiagnosticSeverity::Error,
            source: "bytecode".to_string(),
            code: Some("AmbiguousBytecodeTarget".to_string()),
            message: "More than one Move module matched. Pass --module or --file to choose one."
                .to_string(),
            file: requested_file,
            span: None,
        }),
    }
}

pub fn bytecode_targets(
    context: &CliContext,
    requested_module: Option<&str>,
    requested_file: Option<&str>,
) -> Result<Vec<BytecodeTarget>, CliDiagnostic> {
    let modules = requested_module
        .map(|module| vec![module.to_string()])
        .unwrap_or_default();
    let (package, source, modules) = fetch_modules(
        &context.project_root,
        &context.package_path,
        modules,
        requested_file.map(str::to_string),
    )
    .map_err(|error| CliDiagnostic::error("mcp:peregrine", error))?;
    require_package_source_modules(&package, &source)?;

    Ok(modules
        .into_iter()
        .map(|module| BytecodeTarget {
            source_path: context.project_root.join(&module.file_path),
            file_path: module.file_path,
            module_name: module.module_name,
        })
        .collect())
}

fn require_package_source_modules(
    package: &PackageSummary,
    source: &MoveSourceSummary,
) -> Result<(), CliDiagnostic> {
    if source.has_source_modules {
        return Ok(());
    }

    let code = if source.source_file_count == 0 {
        "NoMoveSources"
    } else {
        "NoMoveSourceModules"
    };
    let message = if source.source_file_count == 0 {
        format!(
            "Move package `{}` ({}) contains a Move.toml manifest but no Move source files under sources/. Call graph, type graph, bytecode, CFG, signatures, and verification require parseable source modules.",
            package.package_name, package.package_path
        )
    } else {
        let noun = if source.source_file_count == 1 {
            "file"
        } else {
            "files"
        };
        format!(
            "Move package `{}` ({}) contains {} Move source {noun}, but no parseable Move modules were found. The source may be commented out or invalid. Call graph, type graph, bytecode, CFG, signatures, and verification require parseable source modules.",
            package.package_name, package.package_path, source.source_file_count
        )
    };

    Err(CliDiagnostic {
        severity: CliDiagnosticSeverity::Error,
        source: "bytecode".to_string(),
        code: Some(code.to_string()),
        message,
        file: Some(source.manifest_path.clone()),
        span: None,
    })
}

fn no_bytecode_target(requested_file: Option<String>) -> CliDiagnostic {
    CliDiagnostic {
        severity: CliDiagnosticSeverity::Error,
        source: "bytecode".to_string(),
        code: Some("NoBytecodeTarget".to_string()),
        message: "No Move module matched the requested bytecode target.".to_string(),
        file: requested_file,
        span: None,
    }
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
            format!("Could not read package path {package_path}: {error}"),
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
    fn discovers_bytecode_target_by_module_name_without_using_file_stem() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::write(
            temp.path().join("sources/not_the_module_name.move"),
            "module demo::actual { public fun ping() {} }",
        )
        .expect("source");
        let context = resolve_context(temp.path(), ".").expect("context");
        let args = BytecodeArgs {
            module: Some("actual".to_string()),
            file: None,
            interactive: false,
            bytecode_map: false,
            debug: false,
        };

        let target = bytecode_target(&context, &args).expect("target");

        assert_eq!(target.module_name, "actual");
        assert_eq!(target.file_path, "sources/not_the_module_name.move");
    }

    #[test]
    fn bytecode_target_reports_manifest_only_package() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"manifest_only\"\n",
        )
        .expect("manifest");
        let context = resolve_context(temp.path(), ".").expect("context");
        let args = BytecodeArgs {
            module: None,
            file: None,
            interactive: false,
            bytecode_map: false,
            debug: false,
        };

        let error = bytecode_target(&context, &args).expect_err("manifest-only package");

        assert_eq!(error.code.as_deref(), Some("NoMoveSources"));
        assert_eq!(error.file.as_deref(), Some("Move.toml"));
        assert!(
            error
                .message
                .contains("no Move source files under sources/")
        );
    }

    #[test]
    fn bytecode_target_reports_source_files_without_parseable_modules() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"generated\"\n",
        )
        .expect("manifest");
        fs::write(
            temp.path().join("sources/generated.move"),
            "/*\nmodule generated::generated;\n*/\n",
        )
        .expect("source");
        let context = resolve_context(temp.path(), ".").expect("context");
        let args = BytecodeArgs {
            module: None,
            file: None,
            interactive: false,
            bytecode_map: false,
            debug: false,
        };

        let error = bytecode_target(&context, &args).expect_err("comment-only package");

        assert_eq!(error.code.as_deref(), Some("NoMoveSourceModules"));
        assert_eq!(error.file.as_deref(), Some("Move.toml"));
        assert!(error.message.contains("no parseable Move modules"));
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
