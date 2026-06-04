use peregrine_adapters::sui::SuiAdapterSettings;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiSecurityToolsConfig {
    pub mode: SuiSecurityToolsMode,
    pub adapter: SuiAdapterSettings,
}

impl Default for SuiSecurityToolsConfig {
    fn default() -> Self {
        Self {
            mode: SuiSecurityToolsMode::Auto,
            adapter: SuiAdapterSettings::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SuiSecurityToolsMode {
    #[default]
    Auto,
    Always,
    Disabled,
}
