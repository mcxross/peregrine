use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiAdapterSettings {
    #[serde(default = "default_sui_adapter_source", alias = "binarySource")]
    pub source: SuiAdapterSource,
    #[serde(default, alias = "binaryPath", alias = "suiCliPath")]
    pub cli_path: Option<String>,
}

impl Default for SuiAdapterSettings {
    fn default() -> Self {
        Self {
            source: SuiAdapterSource::Bundled,
            cli_path: None,
        }
    }
}

impl SuiAdapterSettings {
    pub(crate) fn configured_cli_path(&self) -> Option<&str> {
        self.cli_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
    }
}

fn default_sui_adapter_source() -> SuiAdapterSource {
    SuiAdapterSource::Bundled
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SuiAdapterSource {
    Bundled,
    System,
}

impl SuiAdapterSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::System => "user installed",
        }
    }
}
