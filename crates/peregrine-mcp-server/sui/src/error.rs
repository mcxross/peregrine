use std::path::PathBuf;

pub type SecurityToolsResult<T> = Result<T, SecurityToolsError>;

#[derive(Debug, thiserror::Error)]
pub enum SecurityToolsError {
    #[error("Invalid project root {path}: {reason}")]
    InvalidProjectRoot { path: PathBuf, reason: String },
    #[error("Invalid package path `{path}`: {reason}")]
    InvalidPackagePath { path: String, reason: String },
    #[error("Could not resolve Peregrine helper executable: {0}")]
    HelperExecutable(String),
    #[error("Unsupported security Sui command `{0}`")]
    UnsupportedCommand(String),
    #[error("Sui adapter error: {0}")]
    SuiAdapter(String),
    #[error("{0}")]
    Analysis(String),
}

impl From<peregrine_sui_adapter::SuiAdapterError> for SecurityToolsError {
    fn from(error: peregrine_sui_adapter::SuiAdapterError) -> Self {
        Self::SuiAdapter(error.to_string())
    }
}
