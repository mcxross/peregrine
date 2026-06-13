use serde::{Deserialize, Serialize};

use crate::core::{ChunkId, ContextLevel, PackageId, estimate_tokens};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: ChunkId,
    pub package_id: PackageId,
    pub target_id: String,
    pub level: ContextLevel,
    pub estimated_tokens: usize,
    pub text: String,
    pub metadata_json: Option<serde_json::Value>,
}

pub fn chunk_for_text(
    id: ChunkId,
    package_id: PackageId,
    target_id: String,
    level: ContextLevel,
    text: String,
    metadata_json: Option<serde_json::Value>,
) -> Chunk {
    Chunk {
        id,
        package_id,
        target_id,
        level,
        estimated_tokens: estimate_tokens(&text),
        text,
        metadata_json,
    }
}
