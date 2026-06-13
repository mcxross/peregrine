use serde::{Deserialize, Serialize};

use super::{PackageId, SourceSpan};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTag {
    pub id: String,
    pub package_id: PackageId,
    pub target_id: String,
    pub tag: String,
    pub source_span: SourceSpan,
    pub metadata_json: Option<serde_json::Value>,
}

pub const PROHIBITED_TAG_TERMS: &[&str] = &[
    "vulnerable",
    "safe",
    "unguarded_transfer",
    "missing_authorization",
    "auth_bypass",
    "exploitable",
    "guaranteed_guarded",
];

pub fn is_neutral_tag(tag: &str) -> bool {
    let lower = tag.to_ascii_lowercase();
    !PROHIBITED_TAG_TERMS.iter().any(|term| lower.contains(term))
}
