use std::path::Path;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::core::{hash_file, hash_str, BuildMetadata, IndexerResult};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFingerprints {
    pub move_toml_hash: Option<String>,
    pub package_summaries_hash: Option<String>,
    pub source_hash: Option<String>,
    pub dependency_metadata_hash: Option<String>,
    pub compiler_version: Option<String>,
    pub indexer_version: String,
    pub extraction_config_hash: Option<String>,
}

impl PackageFingerprints {
    pub fn build_metadata(&self) -> BuildMetadata {
        BuildMetadata {
            move_toml_hash: self.move_toml_hash.clone(),
            source_hash: self.source_hash.clone(),
            dependency_metadata_hash: self.dependency_metadata_hash.clone(),
            compiler_version: self.compiler_version.clone(),
            sui_framework_version: None,
            indexer_version: self.indexer_version.clone(),
            extraction_config_hash: self.extraction_config_hash.clone(),
        }
    }
}

pub fn fingerprint_package(
    root: &Path,
    indexer_version: &str,
) -> IndexerResult<PackageFingerprints> {
    let move_toml_hash = optional_hash(root.join("Move.toml").as_path());
    let package_summaries_hash = directory_hash(&root.join("package_summaries"), &["json"])?;
    let source_hash = directory_hash(&root.join("sources"), &["move"])?;
    let dependency_metadata_hash = directory_hash(&root.join("build"), &["json", "lock"])?;
    Ok(PackageFingerprints {
        move_toml_hash,
        package_summaries_hash,
        source_hash,
        dependency_metadata_hash,
        compiler_version: None,
        indexer_version: indexer_version.to_string(),
        extraction_config_hash: None,
    })
}

fn optional_hash(path: &Path) -> Option<String> {
    if path.is_file() {
        hash_file(path).ok()
    } else {
        None
    }
}

fn directory_hash(root: &Path, extensions: &[&str]) -> IndexerResult<Option<String>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut entries = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extensions.iter().any(|allowed| extension == *allowed))
        })
        .collect::<Vec<_>>();
    entries.sort();

    let mut material = String::new();
    for path in entries {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let hash = hash_file(&path)?;
        material.push_str(&relative);
        material.push('\0');
        material.push_str(&hash);
        material.push('\n');
    }
    Ok(Some(hash_str(&material)))
}
