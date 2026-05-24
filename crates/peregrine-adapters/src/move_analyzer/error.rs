use super::MoveAnalyzerAdapterSource;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveAnalyzerAdapterError {
    MissingSystemBinary,
    InvalidExecutionSource {
        expected: MoveAnalyzerAdapterSource,
        actual: MoveAnalyzerAdapterSource,
    },
}

impl fmt::Display for MoveAnalyzerAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystemBinary => {
                write!(
                    formatter,
                    "User installed move-analyzer was not found on PATH. Install move-analyzer or switch to the bundled Move Analyzer in Settings."
                )
            }
            Self::InvalidExecutionSource { expected, actual } => write!(
                formatter,
                "Invalid Move Analyzer execution source: expected {}, got {}.",
                expected.label(),
                actual.label(),
            ),
        }
    }
}

impl std::error::Error for MoveAnalyzerAdapterError {}
