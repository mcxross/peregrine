use crate::{
    BUNDLED_CORPUS,
    index::{KnowledgeIndex, should_index_path},
};
use include_dir::{Dir, DirEntry};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const BUNDLED_PLUGIN_ROOT: &str = "plugins/cache/peregrine-bundled/sui-move-knowledge/local";
const MARKER_FILE: &str = ".peregrine-sui-move-knowledge.marker";
const INSTALLER_SALT: &str = "v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledKnowledgePlugin {
    pub root: PathBuf,
    pub corpus_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct InstallManifest {
    schema_version: u8,
    plugin_id: String,
    package_name: String,
    binary_name: String,
    corpus_hash: String,
    tools: Vec<String>,
    advisory_only: bool,
}

pub fn bundled_cache_root_dir(peregrine_home: &Path) -> PathBuf {
    peregrine_home.join(BUNDLED_PLUGIN_ROOT)
}

pub fn install_bundled_plugin(
    peregrine_home: &Path,
) -> Result<InstalledKnowledgePlugin, KnowledgeInstallError> {
    let index = KnowledgeIndex::bundled()?;
    let root = bundled_cache_root_dir(peregrine_home);
    let marker = marker_for_hash(&index.corpus.corpus_hash);
    if root.is_dir() && read_marker(&root) == Some(marker.clone()) {
        return Ok(InstalledKnowledgePlugin {
            root,
            corpus_hash: index.corpus.corpus_hash,
        });
    }

    let parent = root
        .parent()
        .ok_or(KnowledgeInstallError::InvalidInstallRoot)?;
    fs::create_dir_all(parent)
        .map_err(|source| KnowledgeInstallError::io("create install root", source))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let temporary = parent.join(format!(".sui-move-knowledge-{nonce}.tmp"));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)
            .map_err(|source| KnowledgeInstallError::io("clear temporary install dir", source))?;
    }
    fs::create_dir_all(&temporary)
        .map_err(|source| KnowledgeInstallError::io("create temporary install dir", source))?;

    write_runtime_files(&temporary, &index)?;
    fs::write(temporary.join(MARKER_FILE), format!("{marker}\n"))
        .map_err(|source| KnowledgeInstallError::io("write install marker", source))?;

    replace_dir(&temporary, &root)?;
    Ok(InstalledKnowledgePlugin {
        root,
        corpus_hash: index.corpus.corpus_hash,
    })
}

fn write_runtime_files(root: &Path, index: &KnowledgeIndex) -> Result<(), KnowledgeInstallError> {
    write_json(root.join("index.json"), &index.corpus)?;
    write_json(
        root.join("harness-plugin.json"),
        &InstallManifest {
            schema_version: 1,
            plugin_id: crate::SERVER_NAME.to_string(),
            package_name: env!("CARGO_PKG_NAME").to_string(),
            binary_name: crate::SERVER_BINARY_NAME.to_string(),
            corpus_hash: index.corpus.corpus_hash.clone(),
            tools: vec![
                crate::tool_name::KNOWLEDGE_SEARCH.to_string(),
                crate::tool_name::KNOWLEDGE_READ.to_string(),
                crate::tool_name::SECURITY_RULES.to_string(),
            ],
            advisory_only: true,
        },
    )?;
    write_selected_corpus_files(&BUNDLED_CORPUS, root)?;
    Ok(())
}

fn write_selected_corpus_files(dir: &Dir<'_>, root: &Path) -> Result<(), KnowledgeInstallError> {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(subdir) => write_selected_corpus_files(subdir, root)?,
            DirEntry::File(file) => {
                let relative_path = file.path().to_string_lossy().replace('\\', "/");
                let include_metadata = matches!(
                    relative_path.as_str(),
                    "README.md"
                        | "manifest.json"
                        | "doc-index.json"
                        | "move-security-rules.json"
                        | "audit-context.md"
                );
                if include_metadata || should_index_path(&relative_path) {
                    let path = root.join("knowledge/sui-move").join(&relative_path);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|source| {
                            KnowledgeInstallError::io("create installed corpus directory", source)
                        })?;
                    }
                    fs::write(&path, file.contents()).map_err(|source| {
                        KnowledgeInstallError::io("write installed corpus file", source)
                    })?;
                }
            }
        }
    }
    Ok(())
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<(), KnowledgeInstallError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| KnowledgeInstallError::io("create json parent", source))?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| KnowledgeInstallError::io("write json", source))
}

fn replace_dir(source: &Path, destination: &Path) -> Result<(), KnowledgeInstallError> {
    if !destination.exists() {
        fs::rename(source, destination)
            .map_err(|source| KnowledgeInstallError::io("commit install dir", source))?;
        return Ok(());
    }
    let backup = destination.with_extension("old");
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|source| KnowledgeInstallError::io("clear old install backup", source))?;
    }
    fs::rename(destination, &backup)
        .map_err(|source| KnowledgeInstallError::io("backup existing install dir", source))?;
    if let Err(error) = fs::rename(source, destination) {
        let _ = fs::rename(&backup, destination);
        return Err(KnowledgeInstallError::io("commit install dir", error));
    }
    fs::remove_dir_all(&backup)
        .map_err(|source| KnowledgeInstallError::io("remove install backup", source))
}

fn read_marker(root: &Path) -> Option<String> {
    fs::read_to_string(root.join(MARKER_FILE))
        .ok()
        .map(|value| value.trim().to_string())
}

fn marker_for_hash(corpus_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(INSTALLER_SALT.as_bytes());
    hasher.update(corpus_hash.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Error)]
pub enum KnowledgeInstallError {
    #[error("invalid Sui Move knowledge install root")]
    InvalidInstallRoot,
    #[error(transparent)]
    Index(#[from] crate::index::KnowledgeIndexError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("io error while {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl KnowledgeInstallError {
    fn io(action: &'static str, source: std::io::Error) -> Self {
        Self::Io { action, source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_install_excludes_eval_outputs() {
        let home = tempfile::tempdir().expect("tempdir");

        let installed = install_bundled_plugin(home.path()).expect("install");

        assert!(installed.root.join("index.json").is_file());
        assert!(installed.root.join("harness-plugin.json").is_file());
        assert!(
            !installed
                .root
                .join("knowledge/sui-move/source/move-pr-review/evals")
                .exists()
        );
    }
}
