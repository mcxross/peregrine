use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MoveAnalyzerAdapterSettings {
    #[serde(
        default = "default_move_analyzer_adapter_source",
        alias = "binarySource"
    )]
    pub source: MoveAnalyzerAdapterSource,
    #[serde(
        default,
        alias = "binaryPath",
        alias = "moveAnalyzerPath",
        alias = "serverPath"
    )]
    pub binary_path: Option<String>,
}

impl Default for MoveAnalyzerAdapterSettings {
    fn default() -> Self {
        Self {
            source: MoveAnalyzerAdapterSource::BundledLibrary,
            binary_path: None,
        }
    }
}

impl MoveAnalyzerAdapterSettings {
    pub(crate) fn configured_binary_path(&self) -> Option<&str> {
        self.binary_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
    }
}

fn default_move_analyzer_adapter_source() -> MoveAnalyzerAdapterSource {
    MoveAnalyzerAdapterSource::BundledLibrary
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MoveAnalyzerAdapterSource {
    BundledLibrary,
    System,
}

impl MoveAnalyzerAdapterSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::BundledLibrary => "bundled Move Analyzer",
            Self::System => "user installed Move Analyzer",
        }
    }
}
