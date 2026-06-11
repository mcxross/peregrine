use crate::agent::legacy_core::config::Config;

pub fn get_upgrade_version(_config: &Config) -> Option<String> {
    None
}

pub fn get_upgrade_version_for_popup(_config: &Config) -> Option<String> {
    None
}

pub async fn dismiss_version(_config: &Config, _version: &str) -> anyhow::Result<()> {
    Ok(())
}
