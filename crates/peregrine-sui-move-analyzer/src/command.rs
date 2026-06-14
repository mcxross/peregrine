use super::MoveAnalyzerAdapterSource;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveAnalyzerExecutionTarget {
    BundledLibrary,
    System { executable: PathBuf },
}

impl MoveAnalyzerExecutionTarget {
    pub fn source(&self) -> MoveAnalyzerAdapterSource {
        match self {
            Self::BundledLibrary => MoveAnalyzerAdapterSource::BundledLibrary,
            Self::System { .. } => MoveAnalyzerAdapterSource::System,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveAnalyzerServerCommand {
    pub execution: MoveAnalyzerExecutionTarget,
    pub display: String,
}

impl MoveAnalyzerServerCommand {
    pub(crate) fn new(execution: MoveAnalyzerExecutionTarget) -> Self {
        let display = match &execution {
            MoveAnalyzerExecutionTarget::BundledLibrary => "bundled move-analyzer".to_string(),
            MoveAnalyzerExecutionTarget::System { executable } => {
                executable.to_string_lossy().into_owned()
            }
        };

        Self { execution, display }
    }

    pub fn source(&self) -> MoveAnalyzerAdapterSource {
        self.execution.source()
    }

    pub fn system_executable(&self) -> Option<&Path> {
        match &self.execution {
            MoveAnalyzerExecutionTarget::System { executable } => Some(executable),
            MoveAnalyzerExecutionTarget::BundledLibrary => None,
        }
    }
}
