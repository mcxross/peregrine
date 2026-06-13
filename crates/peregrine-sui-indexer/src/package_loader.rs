use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use peregrine_sui_adapter::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiAdapterSource,
};
use serde_json::json;
use walkdir::WalkDir;

use crate::{
    core::{Diagnostic, DiagnosticSeverity, SourceSpan, hash_file, hash_str, stable_id},
    model::{CompiledPackage, LoadedPackage, SummaryArtifacts},
};

pub fn load_package(root: &Path) -> crate::core::IndexerResult<LoadedPackage> {
    let root = root.canonicalize()?;
    let manifest_path = root.join("Move.toml");
    let manifest_source = fs::read_to_string(&manifest_path)?;
    let package_name = parse_package_name(&manifest_source).unwrap_or_else(|| {
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let manifest_hash = hash_str(&manifest_source);

    Ok(LoadedPackage {
        root,
        manifest_path,
        package_name,
        manifest_hash,
    })
}

pub fn discover_summaries(package: &LoadedPackage) -> crate::core::IndexerResult<SummaryArtifacts> {
    let summary_root = resolve_summary_root(&package.root);
    let mut address_mapping_path = None;
    let mut root_metadata_path = None;
    let mut summary_files = Vec::new();

    if let Some(summary_root) = &summary_root {
        for entry in WalkDir::new(summary_root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                continue;
            }
            match path.file_stem().and_then(|name| name.to_str()) {
                Some("address_mapping") => address_mapping_path = Some(path.to_path_buf()),
                Some("root_package_metadata") => root_metadata_path = Some(path.to_path_buf()),
                _ => summary_files.push(path.to_path_buf()),
            }
        }
    }

    summary_files.sort();

    Ok(SummaryArtifacts {
        package: package.clone(),
        summary_root,
        address_mapping_path,
        root_metadata_path,
        summary_files,
    })
}

pub fn compile_package(package: LoadedPackage) -> crate::core::IndexerResult<CompiledPackage> {
    let build_root = resolve_build_root(&package.root, &package.package_name);
    let diagnostics = if build_root.is_none() {
        vec![Diagnostic {
            id: stable_id(
                "diagnostic",
                [
                    package.package_name.as_str(),
                    "missing-build-artifacts",
                    package.manifest_hash.as_str(),
                ],
            ),
            package_id: String::new(),
            severity: DiagnosticSeverity::Info,
            source: "sui.compiler".to_string(),
            message:
                "Compiled bytecode artifacts were not found; Full mode enrichment was skipped."
                    .to_string(),
            source_span: SourceSpan::unknown(),
            metadata_json: Some(json!({
                "expected": "build/<package>/bytecode_modules"
            })),
        }]
    } else {
        Vec::new()
    };

    Ok(CompiledPackage {
        loaded: package,
        build_root,
        diagnostics,
    })
}

pub fn content_hash_or_empty(path: &Path) -> String {
    hash_file(path).unwrap_or_default()
}

pub fn local_sui_cli_version() -> Option<String> {
    static SUI_CLI_VERSION: OnceLock<Option<String>> = OnceLock::new();
    SUI_CLI_VERSION
        .get_or_init(|| {
            SuiAdapter::new(
                SuiAdapterSettings {
                    source: SuiAdapterSource::System,
                    cli_path: None,
                },
                SuiAdapterEnvironment::default(),
            )
            .status()
            .version
        })
        .clone()
}

fn parse_package_name(source: &str) -> Option<String> {
    let value = toml::from_str::<toml::Value>(source).ok()?;
    value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
}

fn resolve_summary_root(root: &Path) -> Option<PathBuf> {
    let direct = root.join("package_summaries");
    if direct.is_dir() {
        return Some(direct);
    }

    WalkDir::new(root)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("package_summaries"))
        .min_by(|left, right| {
            left.components()
                .count()
                .cmp(&right.components().count())
                .then_with(|| left.cmp(right))
        })
}

fn resolve_build_root(root: &Path, package_name: &str) -> Option<PathBuf> {
    let build_root = root.join("build");
    let preferred = build_root.join(package_name);
    if preferred.join("bytecode_modules").is_dir() {
        return Some(preferred);
    }

    let Ok(entries) = fs::read_dir(&build_root) else {
        return None;
    };

    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("bytecode_modules").is_dir())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}
