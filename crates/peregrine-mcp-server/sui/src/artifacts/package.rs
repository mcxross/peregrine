#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use crate::error::{SecurityToolsError, SecurityToolsResult};
use peregrine_sui_move_model::build_move_package;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageContext {
    pub project_root: PathBuf,
    pub package_root: PathBuf,
    pub package_path: String,
    pub package_name: String,
}

pub fn resolve_move_package(
    project_root: impl AsRef<Path>,
    package_path: Option<&str>,
) -> SecurityToolsResult<MovePackageContext> {
    let project_root = project_root.as_ref();
    let project_root =
        project_root
            .canonicalize()
            .map_err(|error| SecurityToolsError::InvalidProjectRoot {
                path: project_root.to_path_buf(),
                reason: error.to_string(),
            })?;
    if !project_root.is_dir() {
        return Err(SecurityToolsError::InvalidProjectRoot {
            path: project_root,
            reason: "path is not a directory".to_string(),
        });
    }

    let package_path = normalize_package_path(package_path.unwrap_or("."))?;
    let package_root = if package_path == "." {
        project_root.clone()
    } else {
        project_root.join(&package_path)
    };
    let package_root =
        package_root
            .canonicalize()
            .map_err(|error| SecurityToolsError::InvalidPackagePath {
                path: package_path.clone(),
                reason: error.to_string(),
            })?;

    if !package_root.starts_with(&project_root) {
        return Err(SecurityToolsError::InvalidPackagePath {
            path: package_path,
            reason: "package path escapes the project root".to_string(),
        });
    }

    let manifest_path = package_root.join("Move.toml");
    if !manifest_path.is_file() {
        return Err(SecurityToolsError::InvalidPackagePath {
            path: display_package_path(&project_root, &package_root),
            reason: "package does not contain a Move.toml file".to_string(),
        });
    }

    let model = build_move_package(&project_root, &manifest_path, false).ok_or_else(|| {
        SecurityToolsError::InvalidPackagePath {
            path: display_package_path(&project_root, &package_root),
            reason: "could not read package manifest".to_string(),
        }
    })?;
    let package_path = if model.path.is_empty() {
        ".".to_string()
    } else {
        model.path
    };

    Ok(MovePackageContext {
        project_root,
        package_root,
        package_path,
        package_name: model.name,
    })
}

fn normalize_package_path(raw: &str) -> SecurityToolsResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." {
        return Ok(".".to_string());
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(SecurityToolsError::InvalidPackagePath {
            path: trimmed.to_string(),
            reason: "absolute paths are not allowed".to_string(),
        });
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(SecurityToolsError::InvalidPackagePath {
                    path: trimmed.to_string(),
                    reason: "parent directory components are not allowed".to_string(),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(SecurityToolsError::InvalidPackagePath {
                    path: trimmed.to_string(),
                    reason: "absolute paths are not allowed".to_string(),
                });
            }
        }
    }

    if parts.is_empty() {
        Ok(".".to_string())
    } else {
        Ok(parts.join("/"))
    }
}

fn display_package_path(project_root: &Path, package_root: &Path) -> String {
    package_root
        .strip_prefix(project_root)
        .ok()
        .and_then(|path| {
            let value = path
                .components()
                .filter_map(|component| component.as_os_str().to_str())
                .collect::<Vec<_>>()
                .join("/");
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| ".".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_move_package_rejects_escape() {
        let temp = tempdir().expect("tempdir");
        let err = resolve_move_package(temp.path(), Some("../outside")).expect_err("escape");
        assert!(matches!(err, SecurityToolsError::InvalidPackagePath { .. }));
    }
}
