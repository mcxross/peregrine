use sha2::{Digest, Sha256};
use std::{fmt::Write, fs, path::Path};

pub type PackageId = String;
pub type FileId = String;
pub type SummaryArtifactId = String;
pub type ModuleId = String;
pub type TypeId = String;
pub type FieldId = String;
pub type FunctionId = String;
pub type ParameterId = String;
pub type LocalId = String;
pub type BasicBlockId = String;
pub type OperationId = String;
pub type EdgeId = String;
pub type DiagnosticId = String;
pub type ChunkId = String;

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn hash_str(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

pub fn hash_file(path: &Path) -> std::io::Result<String> {
    fs::read(path).map(|bytes| hash_bytes(&bytes))
}

pub fn stable_id(prefix: &str, parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut input = String::new();
    for part in parts {
        let part = part.as_ref();
        let _ = write!(input, "{}:{};", part.len(), part);
    }
    format!("{prefix}:{}", &hash_str(&input)[..24])
}

pub fn logical_id(prefix: &str, parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let encoded = parts
        .into_iter()
        .map(|part| sanitize_id_part(part.as_ref()))
        .collect::<Vec<_>>()
        .join("::");

    if encoded.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}:{encoded}")
    }
}

fn sanitize_id_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '_' | '-' | '.' | ':' | '<' | '>')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}
