mod source_parser;

use serde::Serialize;
use source_parser::{
    discover_modules_from_files, discover_source_files, has_parseable_source_module,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use source_parser::parse_module_declarations;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageModel {
    pub name: String,
    pub path: String,
    pub manifest_path: String,
    pub has_source_files: bool,
    pub has_source_modules: bool,
    pub source_file_count: usize,
    pub modules: Vec<MoveModule>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveModule {
    pub name: String,
    pub address: Option<String>,
    pub file_path: String,
    pub attributes: Vec<String>,
    pub structs: Vec<MoveStructSignature>,
    pub functions: Vec<MoveFunctionSignature>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructSignature {
    pub name: String,
    pub abilities: Vec<String>,
    pub fields: Vec<MoveStructField>,
    pub signature: String,
    pub attributes: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructField {
    pub name: String,
    pub type_name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionSignature {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub signature: String,
    pub body: Option<String>,
    pub attributes: Vec<String>,
}

pub fn discover_move_packages(root: &Path, include_modules: bool) -> Vec<MovePackageModel> {
    let mut manifest_paths = Vec::new();

    collect_move_manifests(root, root, &mut manifest_paths);
    manifest_paths.sort();

    manifest_paths
        .into_iter()
        .filter_map(|manifest_path| build_move_package(root, &manifest_path, include_modules))
        .collect::<Vec<_>>()
}

fn collect_move_manifests(root: &Path, directory: &Path, manifest_paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            if should_skip_project_discovery_dir(&path) {
                continue;
            }

            collect_move_manifests(root, &path, manifest_paths);
            continue;
        }

        if file_type.is_file() && entry.file_name() == "Move.toml" && path.starts_with(root) {
            manifest_paths.push(path);
        }
    }
}

fn should_skip_project_discovery_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    matches!(
        name,
        ".git"
            | ".next"
            | ".sui"
            | ".turbo"
            | "build"
            | "coverage"
            | "dist"
            | "node_modules"
            | "package_summaries"
            | "target"
    )
}

pub fn root_package_name(packages: &[MovePackageModel]) -> Option<String> {
    packages
        .iter()
        .find(|move_package| move_package.path.is_empty())
        .or_else(|| packages.first())
        .map(|move_package| move_package.name.clone())
}

pub fn build_move_package(
    root: &Path,
    manifest_path: &Path,
    include_modules: bool,
) -> Option<MovePackageModel> {
    let package_root = manifest_path.parent()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    let name = package_name(&manifest).unwrap_or_else(|| {
        package_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Move package")
            .to_string()
    });
    let path = relative_path(root, package_root)?;
    let manifest_path = relative_path(root, manifest_path)?;
    let source_files = discover_source_files(package_root);
    let source_file_count = source_files.len();
    let mut modules = if include_modules {
        discover_modules_from_files(root, &source_files)
    } else {
        Vec::new()
    };
    let has_source_modules = if include_modules {
        !modules.is_empty()
    } else {
        has_parseable_source_module(root, &source_files)
    };

    modules.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });

    Some(MovePackageModel {
        name,
        path,
        manifest_path,
        has_source_files: source_file_count > 0,
        has_source_modules,
        source_file_count,
        modules,
    })
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package_section = false;

    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();

        if line.starts_with('[') && line.ends_with(']') {
            in_package_section = line == "[package]";
            continue;
        }

        if !in_package_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if key.trim() != "name" {
            continue;
        }

        return Some(
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        );
    }

    None
}

pub fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}
