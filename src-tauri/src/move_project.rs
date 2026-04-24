use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackage {
    pub name: String,
    pub path: String,
    pub manifest_path: String,
    pub modules: Vec<MoveModule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveModule {
    pub name: String,
    pub address: Option<String>,
    pub file_path: String,
}

pub fn discover_move_packages(root: &Path) -> Vec<MovePackage> {
    let mut manifest_paths = Vec::new();

    collect_move_manifests(root, root, &mut manifest_paths);
    manifest_paths.sort();

    manifest_paths
        .into_iter()
        .filter_map(|manifest_path| build_move_package(root, &manifest_path))
        .collect()
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
            collect_move_manifests(root, &path, manifest_paths);
            continue;
        }

        if file_type.is_file() && entry.file_name() == "Move.toml" && path.starts_with(root) {
            manifest_paths.push(path);
        }
    }
}

fn build_move_package(root: &Path, manifest_path: &Path) -> Option<MovePackage> {
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
    let mut modules = discover_modules(root, package_root);

    modules.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });

    Some(MovePackage {
        name,
        path,
        manifest_path,
        modules,
    })
}

fn discover_modules(root: &Path, package_root: &Path) -> Vec<MoveModule> {
    let sources = package_root.join("sources");
    let mut modules = Vec::new();

    collect_move_modules(root, &sources, &mut modules);
    modules
}

fn collect_move_modules(root: &Path, directory: &Path, modules: &mut Vec<MoveModule>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_modules(root, &path, modules);
            continue;
        }

        if !file_type.is_file() || !is_move_file(&path) {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(module) = parse_module_declaration(&source, root, &path) else {
            continue;
        };

        modules.push(module);
    }
}

fn parse_module_declaration(source: &str, root: &Path, path: &Path) -> Option<MoveModule> {
    for line in source.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        let declaration = line
            .strip_prefix("module ")
            .or_else(|| line.strip_prefix("public module "))?;
        let qualified_name = declaration
            .split(|character: char| character == '{' || character.is_whitespace())
            .next()?
            .trim_end_matches(';');
        let (address, name) = match qualified_name.split_once("::") {
            Some((address, name)) => (Some(address.to_string()), name.to_string()),
            None => (None, qualified_name.to_string()),
        };

        if name.is_empty() {
            return None;
        }

        return Some(MoveModule {
            name,
            address,
            file_path: relative_path(root, path)?,
        });
    }

    None
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

fn is_move_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("move"))
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}
