use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiAdapterSettings {
    #[serde(default = "default_sui_adapter_source", alias = "binarySource")]
    pub source: SuiAdapterSource,
}

impl Default for SuiAdapterSettings {
    fn default() -> Self {
        Self {
            source: SuiAdapterSource::Bundled,
        }
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
