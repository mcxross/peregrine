use codex_utils_absolute_path::AbsolutePathBuf;
use peregrine_app_server_client::{RemoteAppServerEndpoint, app_server_control_socket_path};
use peregrine_utils_home_dir::find_peregrine_home;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentServerTargetConfig {
    pub(crate) mode: AgentServerTargetMode,
    pub(crate) endpoint: Option<String>,
}

impl Default for AgentServerTargetConfig {
    fn default() -> Self {
        Self {
            mode: AgentServerTargetMode::Embedded,
            endpoint: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentServerTargetMode {
    Embedded,
    LocalDaemon,
    Remote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedAgentServerTarget {
    Embedded,
    LocalDaemon { endpoint: RemoteAppServerEndpoint },
    Remote { endpoint: RemoteAppServerEndpoint },
}

impl ResolvedAgentServerTarget {
    pub(crate) fn uses_remote_workspace(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }
}

pub(crate) fn resolve_target(
    config: AgentServerTargetConfig,
) -> Result<ResolvedAgentServerTarget, String> {
    match config.mode {
        AgentServerTargetMode::Embedded => Ok(ResolvedAgentServerTarget::Embedded),
        AgentServerTargetMode::LocalDaemon => {
            let endpoint = resolve_remote_addr(config.endpoint.as_deref().unwrap_or("unix://"))?;
            Ok(ResolvedAgentServerTarget::LocalDaemon { endpoint })
        }
        AgentServerTargetMode::Remote => {
            let endpoint = config
                .endpoint
                .as_deref()
                .ok_or_else(|| "remote app-server target requires an endpoint".to_string())
                .and_then(resolve_remote_addr)?;
            Ok(ResolvedAgentServerTarget::Remote { endpoint })
        }
    }
}

fn remote_addr_has_explicit_port(addr: &str, parsed: &Url) -> bool {
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if parsed.port().is_some() {
        return true;
    }

    let Some((_, rest)) = addr.split_once("://") else {
        return false;
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_and_port)| host_and_port);
    let explicit_default_port = match parsed.scheme() {
        "ws" => 80,
        "wss" => 443,
        _ => return false,
    };
    let expected_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    host_and_port == format!("{expected_host}:{explicit_default_port}")
}

pub(crate) fn resolve_remote_addr(addr: &str) -> Result<RemoteAppServerEndpoint, String> {
    if let Some(socket_path) = addr.strip_prefix("unix://") {
        let socket_path = if socket_path.is_empty() {
            let peregrine_home = find_peregrine_home()
                .map_err(|err| format!("failed to resolve PEREGRINE_HOME: {err}"))?;
            app_server_control_socket_path(&peregrine_home).map_err(|err| err.to_string())?
        } else {
            AbsolutePathBuf::relative_to_current_dir(socket_path).map_err(|err| err.to_string())?
        };
        return Ok(RemoteAppServerEndpoint::UnixSocket { socket_path });
    }

    let parsed = Url::parse(addr).map_err(|_| invalid_remote_addr(addr))?;
    if matches!(parsed.scheme(), "ws" | "wss")
        && parsed.host_str().is_some()
        && remote_addr_has_explicit_port(addr, &parsed)
        && parsed.path() == "/"
        && parsed.query().is_none()
        && parsed.fragment().is_none()
    {
        return Ok(RemoteAppServerEndpoint::WebSocket {
            websocket_url: parsed.to_string(),
            auth_token: None,
        });
    }

    Err(invalid_remote_addr(addr))
}

fn invalid_remote_addr(addr: &str) -> String {
    format!(
        "invalid remote address `{addr}`; expected `ws://host:port`, `wss://host:port`, `unix://`, or `unix://PATH`"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_websocket_with_explicit_port() {
        assert!(matches!(
            resolve_remote_addr("ws://127.0.0.1:4500"),
            Ok(RemoteAppServerEndpoint::WebSocket { .. })
        ));
    }

    #[test]
    fn rejects_websocket_without_explicit_port() {
        assert!(resolve_remote_addr("ws://127.0.0.1").is_err());
    }

    #[test]
    fn resolves_relative_unix_socket() {
        assert!(matches!(
            resolve_remote_addr("unix://codex.sock"),
            Ok(RemoteAppServerEndpoint::UnixSocket { .. })
        ));
    }
}
