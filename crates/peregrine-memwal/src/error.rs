use std::fmt;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemWalError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("compatibility error: {0}")]
    Compatibility(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("signer error: {0}")]
    Signer(String),

    #[error("Sui RPC error: {0}")]
    SuiRpc(#[from] tonic::Status),

    #[error("Seal error: {0}")]
    Seal(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("BCS error: {0}")]
    Bcs(#[from] bcs::Error),

    #[error("semver error: {0}")]
    Semver(#[from] semver::Error),

    #[error("object id parse error: {0}")]
    ObjectIdParse(String),

    #[error("unexpected relayer status {status}: {message}")]
    RelayerStatus {
        status: u16,
        message: String,
        server_code: Option<String>,
        raw: String,
    },

    #[error("job `{job_id}` failed: {message}")]
    JobFailed { job_id: String, message: String },

    #[error("job `{job_id}` not found")]
    JobNotFound { job_id: String },

    #[error("timed out waiting {context} after {timeout:?}")]
    Timeout { context: String, timeout: Duration },

    #[error("Walrus error: {0}")]
    Walrus(String),

    #[error("embedding error: {0}")]
    Embedding(String),
}

impl MemWalError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn compatibility(message: impl Into<String>) -> Self {
        Self::Compatibility(message.into())
    }

    pub fn crypto(message: impl Into<String>) -> Self {
        Self::Crypto(message.into())
    }

    pub fn signer(message: impl Into<String>) -> Self {
        Self::Signer(message.into())
    }

    pub fn sui_rpc(status: tonic::Status) -> Self {
        Self::SuiRpc(status)
    }

    pub fn seal(message: impl Into<String>) -> Self {
        Self::Seal(message.into())
    }

    pub fn object_id_parse(error: impl fmt::Display) -> Self {
        Self::ObjectIdParse(error.to_string())
    }

    pub fn walrus(message: impl Into<String>) -> Self {
        Self::Walrus(message.into())
    }

    pub fn embedding(message: impl Into<String>) -> Self {
        Self::Embedding(message.into())
    }
}
