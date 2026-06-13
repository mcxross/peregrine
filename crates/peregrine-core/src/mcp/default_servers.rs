use std::sync::Arc;

use peregrine_config::codex_compat;
use peregrine_mcp_client::default_peregrine_server;
use peregrine_sui_mcp_protocol::{SERVER_NAME, SuiToolsMode};

use super::McpServerContribution;
use super::McpServerContributionFuture;
use super::McpServerContributor;
use crate::config::Config;

pub(super) fn contributors() -> Vec<Arc<dyn McpServerContributor>> {
    vec![Arc::new(PeregrineDefaultMcpServer)]
}

struct PeregrineDefaultMcpServer;

impl McpServerContributor for PeregrineDefaultMcpServer {
    fn contribute<'a>(&'a self, config: &'a Config) -> McpServerContributionFuture<'a> {
        Box::pin(async move {
            if config.sui_tools.mode == SuiToolsMode::Disabled {
                return vec![McpServerContribution::Remove {
                    name: SERVER_NAME.to_string(),
                }];
            }

            default_server_config(config).map_or_else(Vec::new, |config| {
                vec![McpServerContribution::Default {
                    name: SERVER_NAME.to_string(),
                    config: Box::new(config),
                }]
            })
        })
    }
}

fn default_server_config(config: &Config) -> Option<codex_compat::McpServerConfig> {
    let server = default_peregrine_server(
        config.peregrine_self_exe.as_deref(),
        &config.sui_tools.adapter,
    );
    let servers = std::collections::HashMap::from([(SERVER_NAME.to_string(), server)]);
    codex_compat::mcp_server_config_map_to_codex(&servers).remove(SERVER_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core_plugins::PluginsManager;
    use peregrine_helper_protocol::HELPER_ENV_VAR;
    use peregrine_sui_mcp_protocol::SuiAdapterSettings;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    use crate::config::CONFIG_TOML_FILE;
    use crate::config::ConfigBuilder;
    use crate::config::test_config;
    use crate::mcp::McpManager;

    #[tokio::test]
    async fn default_server_is_contributed_without_explicit_config() {
        let config = test_config().await;
        let manager = McpManager::new(Arc::new(PluginsManager::new(
            config.peregrine_home.to_path_buf(),
        )));

        let servers = manager.runtime_servers(&config).await;
        let server = servers
            .get(SERVER_NAME)
            .expect("default Peregrine MCP server");

        assert!(server.enabled);
    }

    #[tokio::test]
    async fn user_configured_and_default_servers_coexist() {
        let peregrine_home = tempdir().expect("tempdir");
        fs::write(
            peregrine_home.path().join(CONFIG_TOML_FILE),
            r#"
[mcp_servers.docs]
command = "docs-mcp-server"
"#,
        )
        .expect("write config");
        let config = ConfigBuilder::without_managed_config_for_tests()
            .peregrine_home(peregrine_home.path().to_path_buf())
            .fallback_cwd(Some(peregrine_home.path().to_path_buf()))
            .build()
            .await
            .expect("load config");
        let manager = McpManager::new(Arc::new(PluginsManager::new(
            config.peregrine_home.to_path_buf(),
        )));

        let servers = manager.runtime_servers(&config).await;

        assert!(servers.contains_key(SERVER_NAME));
        let docs = servers.get("docs").expect("user-configured docs server");
        let codex_compat::McpServerTransportConfig::Stdio { command, .. } = &docs.transport else {
            panic!("docs MCP server should use stdio");
        };
        assert_eq!(command, "docs-mcp-server");
    }

    #[tokio::test]
    async fn explicitly_disabled_default_server_remains_disabled() {
        let peregrine_home = tempdir().expect("tempdir");
        fs::write(
            peregrine_home.path().join(CONFIG_TOML_FILE),
            r#"
[mcp_servers.peregrine-sui]
command = "custom-peregrine-sui-mcp-server"
enabled = false
"#,
        )
        .expect("write config");
        let config = ConfigBuilder::without_managed_config_for_tests()
            .peregrine_home(peregrine_home.path().to_path_buf())
            .fallback_cwd(Some(peregrine_home.path().to_path_buf()))
            .build()
            .await
            .expect("load config");
        let manager = McpManager::new(Arc::new(PluginsManager::new(
            config.peregrine_home.to_path_buf(),
        )));

        let servers = manager.runtime_servers(&config).await;
        let server = servers
            .get(SERVER_NAME)
            .expect("explicit Peregrine MCP server");
        let codex_compat::McpServerTransportConfig::Stdio { command, .. } = &server.transport
        else {
            panic!("explicit Peregrine MCP server should use stdio");
        };

        assert_eq!(command, "custom-peregrine-sui-mcp-server");
        assert!(!server.enabled);
    }

    #[tokio::test]
    async fn default_server_uses_configured_peregrine_executable_for_sibling_lookup() {
        let directory = tempdir().expect("tempdir");
        let frontend = directory.path().join("peregrine-tui");
        let sidecar = frontend.with_file_name(peregrine_sui_mcp_protocol::SERVER_BINARY_NAME);
        let helper = frontend.with_file_name(peregrine_helper_protocol::helper_binary_file_name());
        fs::write(&frontend, "").expect("write frontend");
        fs::write(&sidecar, "").expect("write sidecar");
        fs::write(&helper, "").expect("write helper");
        let mut config = test_config().await;
        config.peregrine_self_exe = Some(frontend);
        let manager = McpManager::new(Arc::new(PluginsManager::new(
            config.peregrine_home.to_path_buf(),
        )));

        let servers = manager.runtime_servers(&config).await;
        let server = servers
            .get(SERVER_NAME)
            .expect("default Peregrine MCP server");
        let codex_compat::McpServerTransportConfig::Stdio { command, env, .. } = &server.transport
        else {
            panic!("default Peregrine MCP server should use stdio");
        };

        assert_eq!(command, sidecar.to_string_lossy().as_ref());
        assert_eq!(
            env.as_ref()
                .and_then(|env| env.get(HELPER_ENV_VAR))
                .map(String::as_str),
            Some(helper.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn explicit_server_config_overrides_the_default() {
        let mut explicit_server = test_default_server();
        let codex_compat::McpServerTransportConfig::Stdio { command, .. } =
            &mut explicit_server.transport
        else {
            panic!("default Peregrine MCP server should use stdio");
        };
        *command = "/explicit/peregrine-sui-mcp-server".to_string();
        let default_server = test_default_server();
        let mut servers = HashMap::from([(SERVER_NAME.to_string(), explicit_server.clone())]);

        McpManager::apply_to_configured_servers(
            &[McpServerContribution::Default {
                name: SERVER_NAME.to_string(),
                config: Box::new(default_server),
            }],
            &mut servers,
        );

        assert_eq!(
            servers,
            HashMap::from([(SERVER_NAME.to_string(), explicit_server)])
        );
    }

    #[tokio::test]
    async fn disabled_mode_removes_the_default_server() {
        let mut config = test_config().await;
        config.sui_tools.mode = SuiToolsMode::Disabled;
        let manager = McpManager::new(Arc::new(PluginsManager::new(
            config.peregrine_home.to_path_buf(),
        )));

        let servers = manager.runtime_servers(&config).await;

        assert!(!servers.contains_key(SERVER_NAME));
    }

    fn test_default_server() -> codex_compat::McpServerConfig {
        let server = default_peregrine_server(None, &SuiAdapterSettings::default());
        let servers = HashMap::from([(SERVER_NAME.to_string(), server)]);
        codex_compat::mcp_server_config_map_to_codex(&servers)
            .remove(SERVER_NAME)
            .expect("default server config")
    }
}
