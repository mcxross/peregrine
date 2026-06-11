use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use peregrine_config::codex_compat;
use peregrine_config::{
    DEFAULT_MCP_SERVER_ENVIRONMENT_ID, McpServerConfig, McpServerTransportConfig,
};
use peregrine_helper_protocol::{HELPER_ENV_VAR, resolve_helper_executable};
use peregrine_mcp_protocol::{
    SERVER_NAME, SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings,
    SuiSecurityToolsMode, resolve_server_executable,
};

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
            if config.sui_security_tools.mode == SuiSecurityToolsMode::Disabled {
                return vec![McpServerContribution::Remove {
                    name: SERVER_NAME.to_string(),
                }];
            }

            default_server_config(&config.sui_security_tools.adapter).map_or_else(
                Vec::new,
                |config| {
                    vec![McpServerContribution::Default {
                        name: SERVER_NAME.to_string(),
                        config: Box::new(config),
                    }]
                },
            )
        })
    }
}

fn default_server_config(adapter: &SuiAdapterSettings) -> Option<codex_compat::McpServerConfig> {
    let mut env = HashMap::from([(
        SUI_ADAPTER_SOURCE_ENV.to_string(),
        adapter.source.as_str().to_string(),
    )]);
    if let Some(cli_path) = adapter.cli_path.as_deref() {
        env.insert(SUI_CLI_PATH_ENV.to_string(), cli_path.to_string());
    }
    if let Ok(helper) = resolve_helper_executable() {
        env.insert(
            HELPER_ENV_VAR.to_string(),
            helper.to_string_lossy().into_owned(),
        );
    }
    let server = McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: resolve_server_executable().to_string_lossy().into_owned(),
            args: Vec::new(),
            env: Some(env),
            env_vars: Vec::new(),
            cwd: None,
        },
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: Some(Duration::from_secs(20)),
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    };
    let servers = HashMap::from([(SERVER_NAME.to_string(), server)]);
    codex_compat::mcp_server_config_map_to_codex(&servers).remove(SERVER_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core_plugins::PluginsManager;

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

    #[test]
    fn explicit_server_config_overrides_the_default() {
        let mut explicit_server =
            default_server_config(&SuiAdapterSettings::default()).expect("explicit server config");
        let codex_compat::McpServerTransportConfig::Stdio { command, .. } =
            &mut explicit_server.transport
        else {
            panic!("default Peregrine MCP server should use stdio");
        };
        *command = "/explicit/peregrine-mcp-server".to_string();
        let default_server =
            default_server_config(&SuiAdapterSettings::default()).expect("default server config");
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
        config.sui_security_tools.mode = SuiSecurityToolsMode::Disabled;
        let manager = McpManager::new(Arc::new(PluginsManager::new(
            config.peregrine_home.to_path_buf(),
        )));

        let servers = manager.runtime_servers(&config).await;

        assert!(!servers.contains_key(SERVER_NAME));
    }
}
