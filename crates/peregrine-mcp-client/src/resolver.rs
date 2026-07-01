use peregrine_config::{
    CONFIG_TOML_FILE, DEFAULT_MCP_SERVER_ENVIRONMENT_ID, McpServerConfig, McpServerTransportConfig,
    config_toml::ConfigToml,
};
use peregrine_helper_protocol::{
    HELPER_ENV_VAR, resolve_helper_executable, resolve_helper_executable_for_current_exe,
};
use peregrine_sui_mcp_protocol::{
    SERVER_NAME, SERVER_PATH_ENV, SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings,
    SuiAdapterSource, SuiToolsMode, resolve_server_executable_from,
};
use peregrine_sui_move_analyzer_mcp_protocol::{
    ADAPTER_SOURCE_ENV as MOVE_ANALYZER_SOURCE_ENV,
    BINARY_PATH_ENV as MOVE_ANALYZER_BINARY_PATH_ENV, MoveAnalyzerAdapterSettings,
    MoveAnalyzerAdapterSource, MoveAnalyzerToolsMode, SERVER_NAME as MOVE_ANALYZER_SERVER_NAME,
    SERVER_PATH_ENV as MOVE_ANALYZER_SERVER_PATH_ENV,
    resolve_server_executable_from as resolve_move_analyzer_server_executable_from,
};
use std::{
    collections::{BTreeMap, HashMap},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpExecutionOrigin {
    ModelSession,
    Workbench,
    Cli,
}

impl McpExecutionOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelSession => "model-session",
            Self::Workbench => "workbench",
            Self::Cli => "cli",
        }
    }
}

#[derive(Clone, Debug)]
pub struct McpClientOptions {
    pub workspace_root: PathBuf,
    pub peregrine_home: PathBuf,
    pub self_exe: Option<PathBuf>,
    pub origin: McpExecutionOrigin,
}

impl McpClientOptions {
    pub fn new(
        workspace_root: PathBuf,
        peregrine_home: PathBuf,
        origin: McpExecutionOrigin,
    ) -> Self {
        Self {
            workspace_root,
            peregrine_home,
            self_exe: std::env::current_exe().ok(),
            origin,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedMcpConfig {
    pub workspace_root: PathBuf,
    pub servers: BTreeMap<String, McpServerConfig>,
    pub origin: McpExecutionOrigin,
}

pub fn resolve_mcp_config(options: McpClientOptions) -> io::Result<ResolvedMcpConfig> {
    let config = read_user_config(&options.peregrine_home)?;
    let mut servers = config.mcp_servers.into_iter().collect::<BTreeMap<_, _>>();
    let tools = config.tools.clone();
    let (mode, adapter) = sui_settings(tools.clone());
    let (move_analyzer_mode, move_analyzer_adapter) = move_analyzer_settings(tools.clone());
    if mode == SuiToolsMode::Disabled {
        servers.remove(SERVER_NAME);
    } else {
        let port = config.sui_mcp_server_port;
        servers.entry(SERVER_NAME.to_string()).or_insert_with(|| {
            default_peregrine_server(options.self_exe.as_deref(), &adapter, port)
        });
    }
    if move_analyzer_mode == MoveAnalyzerToolsMode::Disabled {
        servers.remove(MOVE_ANALYZER_SERVER_NAME);
    } else {
        servers
            .entry(MOVE_ANALYZER_SERVER_NAME.to_string())
            .or_insert_with(|| {
                default_sui_move_analyzer_server(
                    options.self_exe.as_deref(),
                    &move_analyzer_adapter,
                )
            });
    }
    Ok(ResolvedMcpConfig {
        workspace_root: options.workspace_root,
        servers,
        origin: options.origin,
    })
}

pub fn default_sui_move_analyzer_server(
    self_exe: Option<&Path>,
    adapter: &MoveAnalyzerAdapterSettings,
) -> McpServerConfig {
    let server_executable = resolve_move_analyzer_server_executable_from(
        self_exe,
        std::env::var_os(MOVE_ANALYZER_SERVER_PATH_ENV),
        std::env::var_os("PATH"),
    );
    let helper = resolve_helper(self_exe);
    let mut env = HashMap::from([
        (
            MOVE_ANALYZER_SOURCE_ENV.to_string(),
            adapter.source.as_str().to_string(),
        ),
        ("NO_COLOR".to_string(), "1".to_string()),
        ("CLICOLOR".to_string(), "0".to_string()),
        ("TERM".to_string(), "dumb".to_string()),
    ]);
    if let Some(binary_path) = adapter.binary_path.as_deref() {
        env.insert(
            MOVE_ANALYZER_BINARY_PATH_ENV.to_string(),
            binary_path.to_string(),
        );
    }
    if let Some(helper) = helper {
        env.insert(
            HELPER_ENV_VAR.to_string(),
            helper.to_string_lossy().into_owned(),
        );
    }
    McpServerConfig {
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
        startup_timeout_sec: Some(Duration::from_secs(60)),
        tool_timeout_sec: Some(Duration::from_secs(45)),
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

pub fn default_peregrine_server(
    self_exe: Option<&Path>,
    adapter: &SuiAdapterSettings,
    port: Option<u16>,
) -> McpServerConfig {
    let server_executable = resolve_server_executable_from(
        self_exe,
        std::env::var_os(SERVER_PATH_ENV),
        std::env::var_os("PATH"),
    );
    let helper = resolve_helper(self_exe);
    let mut env = HashMap::from([
        (
            SUI_ADAPTER_SOURCE_ENV.to_string(),
            adapter.source.as_str().to_string(),
        ),
        ("NO_COLOR".to_string(), "1".to_string()),
        ("CLICOLOR".to_string(), "0".to_string()),
        ("TERM".to_string(), "dumb".to_string()),
    ]);
    if let Some(cli_path) = adapter.cli_path.as_deref() {
        env.insert(SUI_CLI_PATH_ENV.to_string(), cli_path.to_string());
    }
    if let Some(helper) = helper {
        env.insert(
            HELPER_ENV_VAR.to_string(),
            helper.to_string_lossy().into_owned(),
        );
    }

    let transport = if let Some(p) = port {
        McpServerTransportConfig::StreamableHttp {
            url: format!("http://127.0.0.1:{p}/messages"),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        }
    } else {
        McpServerTransportConfig::Stdio {
            command: server_executable.to_string_lossy().into_owned(),
            args: Vec::new(),
            env: Some(env),
            env_vars: Vec::new(),
            cwd: None,
        }
    };

    McpServerConfig {
        transport,
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: Some(Duration::from_secs(60)),
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

fn resolve_helper(self_exe: Option<&Path>) -> Option<PathBuf> {
    self_exe
        .map(resolve_helper_executable_for_current_exe)
        .unwrap_or_else(resolve_helper_executable)
        .ok()
}

fn read_user_config(peregrine_home: &Path) -> io::Result<ConfigToml> {
    let path = peregrine_home.join(CONFIG_TOML_FILE);
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(ConfigToml::default()),
        Err(error) => return Err(error),
    };
    toml::from_str(&raw).map_err(|error| io::Error::new(ErrorKind::InvalidData, error))
}

fn sui_settings(
    tools: Option<peregrine_config::config_toml::ToolsToml>,
) -> (SuiToolsMode, SuiAdapterSettings) {
    let Some(sui) = tools.and_then(|tools| tools.sui) else {
        return Default::default();
    };
    let mode = match sui.mode {
        Some(peregrine_config::config_toml::SuiToolsModeToml::Always) => SuiToolsMode::Always,
        Some(peregrine_config::config_toml::SuiToolsModeToml::Disabled) => SuiToolsMode::Disabled,
        Some(peregrine_config::config_toml::SuiToolsModeToml::Auto) | None => SuiToolsMode::Auto,
    };
    let adapter = sui
        .adapter
        .map_or_else(SuiAdapterSettings::default, |adapter| SuiAdapterSettings {
            source: match adapter.source {
                Some(peregrine_config::config_toml::SuiAdapterSourceToml::System) => {
                    SuiAdapterSource::System
                }
                Some(peregrine_config::config_toml::SuiAdapterSourceToml::Bundled) | None => {
                    SuiAdapterSource::Bundled
                }
            },
            cli_path: adapter.cli_path,
        });
    (mode, adapter)
}

fn move_analyzer_settings(
    tools: Option<peregrine_config::config_toml::ToolsToml>,
) -> (MoveAnalyzerToolsMode, MoveAnalyzerAdapterSettings) {
    let Some(config) = tools.and_then(|tools| tools.sui_move_analyzer) else {
        return Default::default();
    };
    let mode = match config.mode {
        Some(peregrine_config::config_toml::SuiToolsModeToml::Always) => {
            MoveAnalyzerToolsMode::Always
        }
        Some(peregrine_config::config_toml::SuiToolsModeToml::Disabled) => {
            MoveAnalyzerToolsMode::Disabled
        }
        Some(peregrine_config::config_toml::SuiToolsModeToml::Auto) | None => {
            MoveAnalyzerToolsMode::Auto
        }
    };
    let adapter = config
        .adapter
        .map_or_else(MoveAnalyzerAdapterSettings::default, |adapter| {
            MoveAnalyzerAdapterSettings {
                source: match adapter.source {
                    Some(peregrine_config::config_toml::SuiAdapterSourceToml::System) => {
                        MoveAnalyzerAdapterSource::System
                    }
                    Some(peregrine_config::config_toml::SuiAdapterSourceToml::Bundled) | None => {
                        MoveAnalyzerAdapterSource::Bundled
                    }
                },
                binary_path: adapter.binary_path,
            }
        });
    (mode, adapter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn explicit_servers_override_and_coexist_with_default() -> Result<(), Box<dyn std::error::Error>>
    {
        let home = tempdir()?;
        std::fs::write(
            home.path().join(CONFIG_TOML_FILE),
            r#"
[mcp_servers.docs]
command = "docs-server"

[mcp_servers.peregrine-sui]
command = "custom-server"
enabled = false
"#,
        )?;
        let resolved = resolve_mcp_config(McpClientOptions {
            workspace_root: home.path().to_path_buf(),
            peregrine_home: home.path().to_path_buf(),
            self_exe: None,
            origin: McpExecutionOrigin::Workbench,
        })?;

        assert_eq!(resolved.servers.len(), 4);
        assert!(!resolved.servers[SERVER_NAME].enabled);
        let McpServerTransportConfig::Stdio { command, .. } =
            &resolved.servers[SERVER_NAME].transport
        else {
            panic!("stdio");
        };
        assert_eq!(command, "custom-server");
        Ok(())
    }

    #[test]
    fn disabled_sui_tools_remove_default_server() -> Result<(), Box<dyn std::error::Error>> {
        let home = tempdir()?;
        std::fs::write(
            home.path().join(CONFIG_TOML_FILE),
            "[tools.sui]\nmode = \"disabled\"\n",
        )?;
        let resolved = resolve_mcp_config(McpClientOptions::new(
            home.path().to_path_buf(),
            home.path().to_path_buf(),
            McpExecutionOrigin::Cli,
        ))?;

        assert!(!resolved.servers.contains_key(SERVER_NAME));
        assert!(resolved.servers.contains_key(MOVE_ANALYZER_SERVER_NAME));

        Ok(())
    }

    #[test]
    fn disabled_move_analyzer_tools_remove_only_the_analyzer_server()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = tempdir()?;
        std::fs::write(
            home.path().join(CONFIG_TOML_FILE),
            "[tools.sui_move_analyzer]\nmode = \"disabled\"\n",
        )?;
        let resolved = resolve_mcp_config(McpClientOptions::new(
            home.path().to_path_buf(),
            home.path().to_path_buf(),
            McpExecutionOrigin::Cli,
        ))?;

        assert!(resolved.servers.contains_key(SERVER_NAME));
        assert!(!resolved.servers.contains_key(MOVE_ANALYZER_SERVER_NAME));

        Ok(())
    }

    #[test]
    fn all_bundled_peregrine_servers_are_registered_by_default()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = tempdir()?;
        let resolved = resolve_mcp_config(McpClientOptions::new(
            home.path().to_path_buf(),
            home.path().to_path_buf(),
            McpExecutionOrigin::Workbench,
        ))?;

        assert!(resolved.servers.contains_key(SERVER_NAME));
        assert!(resolved.servers.contains_key(MOVE_ANALYZER_SERVER_NAME));

        Ok(())
    }

    #[test]
    fn missing_sidecars_resolve_to_dedicated_sibling_paths() {
        let directory = tempdir().expect("tempdir");
        let frontend = directory.path().join("peregrine-tui");
        std::fs::write(&frontend, "").expect("write frontend");

        let sui = default_peregrine_server(Some(&frontend), &SuiAdapterSettings::default(), None);
        let analyzer = default_sui_move_analyzer_server(
            Some(&frontend),
            &MoveAnalyzerAdapterSettings::default(),
        );
        assert_dedicated_server(
            &sui,
            &frontend.with_file_name(peregrine_sui_mcp_protocol::SERVER_BINARY_NAME),
        );
        assert_dedicated_server(
            &analyzer,
            &frontend.with_file_name(peregrine_sui_move_analyzer_mcp_protocol::SERVER_BINARY_NAME),
        );
    }

    fn assert_dedicated_server(config: &McpServerConfig, expected: &Path) {
        let McpServerTransportConfig::Stdio {
            command, args, env, ..
        } = &config.transport
        else {
            panic!("MCP server should use stdio");
        };

        assert_eq!(command, expected.to_string_lossy().as_ref());
        assert!(args.is_empty());
        assert!(
            env.as_ref()
                .and_then(|env| env.get(HELPER_ENV_VAR))
                .is_none()
        );
    }
}
