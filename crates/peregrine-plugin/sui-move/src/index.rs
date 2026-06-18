use bm25::{Document, Language, SearchEngine, SearchEngineBuilder};
use include_dir::{Dir, DirEntry};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub const CORPUS_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_SEARCH_RESULTS: usize = 10;
pub const MAX_CHUNK_TOKENS: usize = 900;
pub const MAX_RESPONSE_TOKENS: usize = 3_000;

const CHUNK_OVERLAP_TOKENS: usize = 80;
const SNIPPET_TOKENS: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CorpusIndex {
    pub schema_version: u8,
    pub corpus_name: String,
    pub corpus_version: String,
    pub corpus_hash: String,
    pub chunks: Vec<KnowledgeChunk>,
}

impl CorpusIndex {
    pub fn chunk(&self, chunk_id: &str) -> Option<&KnowledgeChunk> {
        self.chunks.iter().find(|chunk| chunk.id == chunk_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunk {
    pub id: String,
    pub source_path: String,
    pub title: String,
    pub chunk_index: usize,
    pub token_count: usize,
    pub provenance: String,
    pub trust_tier: TrustTier,
    pub topics: Vec<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrustTier {
    Official,
    Curated,
    Example,
    Advisory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub chunk_id: String,
    pub score: f32,
    pub title: String,
    pub source_path: String,
    pub provenance: String,
    pub trust_tier: TrustTier,
    pub topics: Vec<String>,
    pub snippet: String,
}

pub struct KnowledgeIndex {
    pub corpus: CorpusIndex,
    search_engine: SearchEngine<usize>,
}

impl KnowledgeIndex {
    pub fn bundled() -> Result<Self, KnowledgeIndexError> {
        Self::from_dir(&crate::BUNDLED_CORPUS)
    }

    pub fn from_corpus(corpus: CorpusIndex) -> Self {
        let documents = chunks_to_documents(&corpus.chunks);
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
        Self {
            corpus,
            search_engine,
        }
    }

    pub fn from_dir(dir: &Dir<'_>) -> Result<Self, KnowledgeIndexError> {
        let mut files = Vec::new();
        collect_indexed_files(dir, &mut files);
        files.sort_unstable_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let corpus_hash = corpus_hash(&files);
        let topics = doc_topics(dir);
        let mut chunks = Vec::new();
        for file in files {
            let file_topics = topics.get(&file.relative_path).cloned().unwrap_or_default();
            chunks.extend(chunk_file(file, &file_topics));
        }
        Ok(Self::from_corpus(CorpusIndex {
            schema_version: 1,
            corpus_name: "peregrine-sui-move-knowledge".to_string(),
            corpus_version: CORPUS_VERSION.to_string(),
            corpus_hash,
            chunks,
        }))
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let limit = limit.clamp(1, MAX_SEARCH_RESULTS);
        self.search_engine
            .search(query, limit)
            .into_iter()
            .filter_map(|result| {
                self.corpus
                    .chunks
                    .get(result.document.id)
                    .map(|chunk| SearchResult {
                        chunk_id: chunk.id.clone(),
                        score: result.score,
                        title: chunk.title.clone(),
                        source_path: chunk.source_path.clone(),
                        provenance: chunk.provenance.clone(),
                        trust_tier: chunk.trust_tier.clone(),
                        topics: chunk.topics.clone(),
                        snippet: first_tokens(&chunk.content, SNIPPET_TOKENS),
                    })
            })
            .collect()
    }
}

fn chunks_to_documents(chunks: &[KnowledgeChunk]) -> Vec<Document<usize>> {
    chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| Document::new(index, search_text(chunk)))
        .collect()
}

#[derive(Debug, Error)]
pub enum KnowledgeIndexError {
    #[error("bundled corpus file `{0}` is not valid UTF-8")]
    InvalidUtf8(String),
    #[error("bundled corpus metadata is invalid: {0}")]
    InvalidMetadata(String),
}

struct IndexedFile {
    relative_path: String,
    contents: String,
}

fn collect_indexed_files(dir: &Dir<'_>, files: &mut Vec<IndexedFile>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(subdir) => {
                collect_indexed_files(subdir, files);
            }
            DirEntry::File(file) => {
                let relative_path = normalize_path(&file.path().to_string_lossy());
                if should_index_path(&relative_path)
                    && let Ok(contents) = std::str::from_utf8(file.contents())
                {
                    files.push(IndexedFile {
                        relative_path,
                        contents: contents.to_string(),
                    });
                }
            }
        }
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn should_index_path(path: &str) -> bool {
    let path = normalize_path(path);
    let lower = path.to_ascii_lowercase();
    if lower.contains("/evals/")
        || lower.contains("/outputs/")
        || lower.contains("/.raw/")
        || lower.contains("/raw/")
        || lower.contains("grading")
        || lower.contains("benchmark")
        || lower.contains("agent-report")
        || lower.contains("subagent-")
        || lower.ends_with("move.lock")
    {
        return false;
    }

    matches!(
        Path::new(&lower)
            .extension()
            .and_then(|value| value.to_str()),
        Some("md" | "mdx" | "move" | "toml")
    )
}

fn corpus_hash(files: &[IndexedFile]) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(file.relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(file.contents.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn doc_topics(dir: &Dir<'_>) -> BTreeMap<String, Vec<String>> {
    let Some(file) = dir.get_file("doc-index.json") else {
        return BTreeMap::new();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(file.contents()) else {
        return BTreeMap::new();
    };
    value
        .get("documents")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let local_path = item.get("localPath")?.as_str()?;
            let topics = item
                .get("topics")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|topic| topic.as_str().map(str::to_string))
                .collect::<Vec<_>>();
            Some((normalize_doc_index_path(local_path), topics))
        })
        .collect()
}

fn normalize_doc_index_path(path: &str) -> String {
    path.strip_prefix("knowledge/sui-move/")
        .unwrap_or(path)
        .to_string()
}

fn chunk_file(file: IndexedFile, topics: &[String]) -> Vec<KnowledgeChunk> {
    let tokens = file.contents.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return Vec::new();
    }
    let title = title_for_file(&file.relative_path, &file.contents);
    let provenance = provenance_for_path(&file.relative_path);
    let trust_tier = trust_tier_for_path(&file.relative_path);
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut chunk_index = 0;
    while start < tokens.len() {
        let end = (start + MAX_CHUNK_TOKENS).min(tokens.len());
        let content = tokens[start..end].join(" ");
        chunks.push(KnowledgeChunk {
            id: chunk_id(&file.relative_path, chunk_index),
            source_path: file.relative_path.clone(),
            title: title.clone(),
            chunk_index,
            token_count: end - start,
            provenance: provenance.clone(),
            trust_tier: trust_tier.clone(),
            topics: topics.to_vec(),
            content,
        });
        if end == tokens.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP_TOKENS);
        chunk_index += 1;
    }
    chunks
}

fn chunk_id(relative_path: &str, chunk_index: usize) -> String {
    let digest = Sha256::digest(relative_path.as_bytes());
    let short = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sui-move:{short}:{chunk_index}")
}

fn title_for_file(relative_path: &str, contents: &str) -> String {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            Path::new(relative_path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(relative_path)
                .to_string()
        })
}

fn provenance_for_path(path: &str) -> String {
    if path.starts_with("source/sui-docs/") {
        "Sui documentation".to_string()
    } else if path.starts_with("source/move-book-docs/") {
        "Move Book documentation".to_string()
    } else if path.starts_with("source/sui-prover-docs/") {
        "Sui Prover documentation".to_string()
    } else if path.starts_with("source/move-") {
        "Curated Move audit skill".to_string()
    } else {
        "Bundled Peregrine Sui Move knowledge".to_string()
    }
}

fn trust_tier_for_path(path: &str) -> TrustTier {
    if path.starts_with("source/sui-docs/")
        || path.starts_with("source/move-book-docs/book/")
        || path.starts_with("source/move-book-docs/reference/")
        || path.starts_with("source/sui-prover-docs/guide/")
    {
        TrustTier::Official
    } else if path.contains("/examples/") || path.contains("/packages/") || path.ends_with(".move")
    {
        TrustTier::Example
    } else if path.starts_with("source/move-") {
        TrustTier::Curated
    } else {
        TrustTier::Advisory
    }
}

fn search_text(chunk: &KnowledgeChunk) -> String {
    format!(
        "{} {} {} {}",
        chunk.title,
        chunk.source_path,
        chunk.topics.join(" "),
        chunk.content
    )
}

pub fn first_tokens(text: &str, max_tokens: usize) -> String {
    text.split_whitespace()
        .take(max_tokens)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn token_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn response_within_cap(text: &str) -> bool {
    token_count(text) <= MAX_RESPONSE_TOKENS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_outputs_are_not_indexed() {
        assert!(!should_index_path(
            "source/move-pr-review/evals/iteration-1/outputs/raw/reviewer.md"
        ));
        assert!(!should_index_path(
            "source/move-pr-review/evals/iteration-1/with_skill/run-1/grading.json"
        ));
        assert!(!should_index_path(
            "source/move-pr-review/evals/iteration-1/outputs/.raw/subagent-1.json"
        ));
    }

    #[test]
    fn bundled_index_is_deterministic_and_bounded() {
        let first = KnowledgeIndex::bundled().expect("first index");
        let second = KnowledgeIndex::bundled().expect("second index");

        assert_eq!(first.corpus.corpus_hash, second.corpus.corpus_hash);
        assert_eq!(first.corpus.chunks, second.corpus.chunks);
        assert!(
            first
                .corpus
                .chunks
                .iter()
                .all(|chunk| chunk.token_count <= MAX_CHUNK_TOKENS)
        );
        assert!(
            first
                .corpus
                .chunks
                .iter()
                .all(|chunk| !chunk.source_path.contains("/evals/"))
        );
    }

    #[test]
    fn search_limit_is_capped() {
        let index = KnowledgeIndex::bundled().expect("index");

        let results = index.search("shared object access control", 100);

        assert!(results.len() <= MAX_SEARCH_RESULTS);
    }
}
