use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct RememberRequest<'a> {
    pub text: &'a str,
    pub namespace: &'a str,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct RememberAcceptedResult {
    pub job_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct RememberJobStatus {
    pub job_id: String,
    pub status: RememberJobState,
    pub owner: Option<String>,
    pub namespace: Option<String>,
    pub blob_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RememberJobState {
    Pending,
    Running,
    Uploaded,
    Done,
    Failed,
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RememberResult {
    pub id: String,
    pub job_id: String,
    pub blob_id: String,
    pub owner: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RememberBulkRequest<'a> {
    pub items: &'a [RememberBulkItem],
}

#[derive(Clone, Debug, Serialize)]
pub struct RememberBulkItem {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RememberBulkAcceptedResult {
    pub job_ids: Vec<String>,
    pub total: usize,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RememberBulkStatusRequest<'a> {
    pub job_ids: &'a [String],
}

#[derive(Clone, Debug, Deserialize)]
pub struct RememberBulkStatusItem {
    pub job_id: String,
    pub status: RememberJobState,
    pub blob_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RememberBulkStatusResult {
    pub results: Vec<RememberBulkStatusItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RememberBulkItemResult {
    pub id: String,
    pub blob_id: String,
    pub status: BulkCompletionState,
    pub namespace: String,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkCompletionState {
    Done,
    Failed,
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RememberBulkResult {
    pub results: Vec<RememberBulkItemResult>,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecallRequest<'a> {
    pub query: &'a str,
    pub limit: usize,
    pub namespace: &'a str,
}

#[derive(Clone, Debug, Default)]
pub struct RecallParams {
    pub query: String,
    pub limit: Option<usize>,
    pub top_k: Option<usize>,
    pub namespace: Option<String>,
    pub max_distance: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RecallMemory {
    pub blob_id: String,
    pub text: String,
    pub distance: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RecallResult {
    pub results: Vec<RecallMemory>,
    pub total: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq)]
pub struct ScoringWeights {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recency: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "recency_half_life_days"
    )]
    pub recency_half_life_days: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EmbedResult {
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct AnalyzeOptions {
    pub namespace: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnalyzedFact {
    pub text: String,
    pub id: String,
    pub job_id: Option<String>,
    pub blob_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnalyzeResult {
    pub job_ids: Vec<String>,
    pub facts: Vec<AnalyzedFact>,
    pub fact_count: usize,
    pub status: String,
    pub owner: String,
}

#[derive(Clone, Debug)]
pub struct AnalyzeWaitResult {
    pub results: RememberBulkResult,
    pub facts: Vec<AnalyzedFact>,
    pub owner: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RestoreResult {
    pub restored: usize,
    pub skipped: usize,
    pub total: usize,
    pub namespace: String,
    pub owner: String,
}
