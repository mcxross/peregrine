use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::config::Config;
use codex_core_plugins::PluginsManager;
use codex_login::CodexAuth;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConfig;
use codex_mcp::ToolPluginProvenance;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;
use codex_mcp::tool_plugin_provenance as collect_tool_plugin_provenance;
use peregrine_config::codex_compat;

mod default_servers;

#[derive(Clone)]
pub struct McpManager {
    plugins_manager: Arc<PluginsManager>,
    contributors: Arc<Vec<Arc<dyn McpServerContributor>>>,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self::new_with_contributors(plugins_manager, default_servers::contributors())
    }

    fn new_with_contributors(
        plugins_manager: Arc<PluginsManager>,
        contributors: Vec<Arc<dyn McpServerContributor>>,
    ) -> Self {
        Self {
            plugins_manager,
            contributors: Arc::new(contributors),
        }
    }

    pub async fn runtime_config(&self, config: &Config) -> McpConfig {
        let mut mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        let explicitly_disabled = mcp_config
            .configured_mcp_servers
            .iter()
            .filter(|(_, server)| !server.enabled)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();

        let contributions = self.contributions(config).await;
        Self::apply_to_configured_servers(&contributions, &mut mcp_config.configured_mcp_servers);
        for name in explicitly_disabled {
            if let Some(server) = mcp_config.configured_mcp_servers.get_mut(&name) {
                server.enabled = false;
            }
        }
        mcp_config
    }

    pub async fn configured_servers(
        &self,
        config: &Config,
    ) -> HashMap<String, codex_compat::McpServerConfig> {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        configured_mcp_servers(&mcp_config)
    }

    pub async fn runtime_servers(
        &self,
        config: &Config,
    ) -> HashMap<String, codex_compat::McpServerConfig> {
        let mcp_config = self.runtime_config(config).await;
        configured_mcp_servers(&mcp_config)
    }

    pub async fn effective_servers(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> HashMap<String, EffectiveMcpServer> {
        let mcp_config = self.runtime_config(config).await;
        effective_mcp_servers(&mcp_config, auth)
    }

    pub async fn tool_plugin_provenance(&self, config: &Config) -> ToolPluginProvenance {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        collect_tool_plugin_provenance(&mcp_config)
    }

    async fn contributions(&self, config: &Config) -> Vec<McpServerContribution> {
        let mut contributions = Vec::new();
        for contributor in self.contributors.iter() {
            contributions.extend(contributor.contribute(config).await);
        }
        contributions
    }

    fn apply_to_configured_servers(
        contributions: &[McpServerContribution],
        servers: &mut HashMap<String, codex_compat::McpServerConfig>,
    ) {
        for contribution in contributions {
            match contribution {
                McpServerContribution::Default { name, config } => {
                    servers
                        .entry(name.clone())
                        .or_insert_with(|| config.as_ref().clone());
                }
                McpServerContribution::Remove { name } => {
                    servers.remove(name);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
enum McpServerContribution {
    Default {
        name: String,
        config: Box<codex_compat::McpServerConfig>,
    },
    Remove {
        name: String,
    },
}

type McpServerContributionFuture<'a> =
    Pin<Box<dyn Future<Output = Vec<McpServerContribution>> + Send + 'a>>;

/// Resolves host-owned MCP server contributions from the effective application
/// configuration. Default contributions must remain overridable by explicit
/// user configuration with the same server name.
trait McpServerContributor: Send + Sync {
    fn contribute<'a>(&'a self, config: &'a Config) -> McpServerContributionFuture<'a>;
}
