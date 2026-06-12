use peregrine_config::{
    CONFIG_TOML_FILE, DEFAULT_MCP_SERVER_ENVIRONMENT_ID, McpServerConfig, McpServerTransportConfig,
    config_toml::ConfigToml,
};
use peregrine_helper_protocol::{
    HELPER_ENV_VAR, resolve_helper_executable, resolve_helper_executable_for_current_exe,
};
use peregrine_mcp_protocol::{
    SERVER_NAME, SERVER_PATH_ENV, SUI_ADAPTER_SOURCE_ENV, SUI_CLI_PATH_ENV, SuiAdapterSettings,
    SuiAdapterSource, SuiSecurityToolsMode, resolve_server_executable_from,
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
    let (mode, adapter) = sui_settings(config.tools);

    if mode == SuiSecurityToolsMode::Disabled {
        servers.remove(SERVER_NAME);
    } else {
        servers
            .entry(SERVER_NAME.to_string())
            .or_insert_with(|| default_peregrine_server(options.self_exe.as_deref(), &adapter));
    }

    Ok(ResolvedMcpConfig {
        workspace_root: options.workspace_root,
        servers,
        origin: options.origin,
    })
}

pub fn default_peregrine_server(
    self_exe: Option<&Path>,
    adapter: &SuiAdapterSettings,
) -> McpServerConfig {
    let server_executable = resolve_server_executable_from(
        self_exe,
        std::env::var_os(SERVER_PATH_ENV),
        std::env::var_os("PATH"),
    );
    let helper = self_exe
        .map(resolve_helper_executable_for_current_exe)
        .unwrap_or_else(resolve_helper_executable)
        .ok();
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
        startup_timeout_sec: Some(Duration::from_secs(20)),
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
) -> (SuiSecurityToolsMode, SuiAdapterSettings) {
    let Some(sui) = tools.and_then(|tools| tools.sui_security) else {
        return Default::default();
    };
    let mode = match sui.mode {
        Some(peregrine_config::config_toml::SuiSecurityToolsModeToml::Always) => {
            SuiSecurityToolsMode::Always
        }
        Some(peregrine_config::config_toml::SuiSecurityToolsModeToml::Disabled) => {
            SuiSecurityToolsMode::Disabled
        }
        Some(peregrine_config::config_toml::SuiSecurityToolsModeToml::Auto) | None => {
            SuiSecurityToolsMode::Auto
        }
    };
    let adapter = sui
        .adapter
        .map_or_else(SuiAdapterSettings::default, |adapter| SuiAdapterSettings {
            source: match adapter.source {
                Some(peregrine_config::config_toml::SuiSecurityAdapterSourceToml::System) => {
                    SuiAdapterSource::System
                }
                Some(peregrine_config::config_toml::SuiSecurityAdapterSourceToml::Bundled)
                | None => SuiAdapterSource::Bundled,
            },
            cli_path: adapter.cli_path,
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

[mcp_servers.peregrine]
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

        assert_eq!(resolved.servers.len(), 2);
        assert!(!resolved.servers["peregrine"].enabled);
        let McpServerTransportConfig::Stdio { command, .. } =
            &resolved.servers["peregrine"].transport
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
            "[tools.sui_security]\nmode = \"disabled\"\n",
        )?;
        let resolved = resolve_mcp_config(McpClientOptions::new(
            home.path().to_path_buf(),
            home.path().to_path_buf(),
            McpExecutionOrigin::Cli,
        ))?;

        assert!(!resolved.servers.contains_key(SERVER_NAME));
        Ok(())
    }
}
