use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use peregrine_config::codex_compat;
use peregrine_config::{
    DEFAULT_MCP_SERVER_ENVIRONMENT_ID, McpServerConfig, McpServerTransportConfig,
};
use peregrine_helper_protocol::{
    HELPER_ENV_VAR, resolve_helper_executable, resolve_helper_executable_for_current_exe,
};
use peregrine_mcp_protocol::{
    SERVER_NAME, SERVER_PATH_ENV, SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings,
    SuiSecurityToolsMode, resolve_server_executable_from,
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
    let helper = config
        .peregrine_self_exe
        .as_deref()
        .map(resolve_helper_executable_for_current_exe)
        .unwrap_or_else(resolve_helper_executable)
        .ok();
    default_server_config_from_parts(
        &config.sui_security_tools.adapter,
        resolve_server_executable_from(
            config.peregrine_self_exe.as_deref(),
            std::env::var_os(SERVER_PATH_ENV),
            std::env::var_os("PATH"),
        ),
        helper,
    )
}

fn default_server_config_from_parts(
    adapter: &SuiAdapterSettings,
    server_executable: std::path::PathBuf,
    helper: Option<std::path::PathBuf>,
) -> Option<codex_compat::McpServerConfig> {
    let mut env = HashMap::from([(
        SUI_ADAPTER_SOURCE_ENV.to_string(),
        adapter.source.as_str().to_string(),
    )]);
    if let Some(cli_path) = adapter.cli_path.as_deref() {
        env.insert(SUI_CLI_PATH_ENV.to_string(), cli_path.to_string());
    }
    if let Some(helper) = helper {
        env.insert(
            HELPER_ENV_VAR.to_string(),
            helper.to_string_lossy().into_owned(),
        );
    }
    let server = McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: server_executable.to_string_lossy().into_owned(),
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
[mcp_servers.peregrine]
command = "custom-peregrine-mcp-server"
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

        assert_eq!(command, "custom-peregrine-mcp-server");
        assert!(!server.enabled);
    }

    #[tokio::test]
    async fn default_server_uses_configured_peregrine_executable_for_sibling_lookup() {
        let directory = tempdir().expect("tempdir");
        let frontend = directory.path().join("peregrine-tui");
        let sidecar = frontend.with_file_name(peregrine_mcp_protocol::SERVER_BINARY_NAME);
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
        let mut explicit_server = default_server_config_from_parts(
            &SuiAdapterSettings::default(),
            "/default/peregrine-mcp-server".into(),
            None,
        )
        .expect("explicit server config");
        let codex_compat::McpServerTransportConfig::Stdio { command, .. } =
            &mut explicit_server.transport
        else {
            panic!("default Peregrine MCP server should use stdio");
        };
        *command = "/explicit/peregrine-mcp-server".to_string();
        let default_server = default_server_config_from_parts(
            &SuiAdapterSettings::default(),
            "/default/peregrine-mcp-server".into(),
            None,
        )
        .expect("default server config");
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
