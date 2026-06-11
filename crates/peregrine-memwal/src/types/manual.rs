use serde::Deserialize;
use serde::Serialize;

use crate::types::ScoringWeights;

#[derive(Clone, Debug, Serialize)]
pub struct RegisterMemoryRequest<'a> {
    pub blob_id: &'a str,
    pub vector: &'a [f32],
    pub namespace: &'a str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManualEncryptedRegisterRequest<'a> {
    pub encrypted_data: &'a str,
    pub vector: &'a [f32],
    pub namespace: &'a str,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RememberManualResult {
    pub id: String,
    pub blob_id: String,
    pub owner: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecallVectorRequest<'a> {
    pub vector: &'a [f32],
    pub limit: usize,
    pub namespace: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scoring_weights: Option<&'a ScoringWeights>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RecallManualHit {
    pub blob_id: String,
    pub distance: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RecallManualResult {
    pub results: Vec<RecallManualHit>,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ManualRecallMemory {
    pub blob_id: String,
    pub text: String,
    pub distance: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManualRecallFailureStage {
    Download,
    Decode,
    Decrypt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ManualRecallFailure {
    pub blob_id: String,
    pub stage: ManualRecallFailureStage,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ManualRecallResultWithFailures {
    pub results: Vec<ManualRecallMemory>,
    pub total: usize,
    pub failures: Vec<ManualRecallFailure>,
}

#[derive(Clone, Debug, Default)]
pub struct ManualRecallOptions {
    pub limit: Option<usize>,
    pub namespace: Option<String>,
    pub scoring_weights: Option<ScoringWeights>,
}

#[derive(Clone, Debug)]
pub struct SealServerConfig {
    pub object_id: sui_sdk_types::Address,
    pub weight: u8,
    pub aggregator_url: Option<String>,
    pub api_key_name: Option<String>,
    pub api_key: Option<String>,
}
