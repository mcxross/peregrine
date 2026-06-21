use super::events;
use super::models::{AgentServerStartRequest, AgentServerStartResponse};
use super::target::{AgentServerTargetConfig, ResolvedAgentServerTarget, resolve_target};
use codex_arg0::Arg0DispatchPaths;
use codex_exec_server::{EnvironmentManager, ExecServerRuntimePaths};
use codex_feedback::CodexFeedback;
use codex_rollout::state_db;
use peregrine_app_server_client::{
    AppServerClient, AppServerRequestHandle, DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    InProcessAppServerClient, InProcessClientStartArgs, RemoteAppServerClient,
    RemoteAppServerConnectArgs,
};
use peregrine_app_server_protocol::{
    ClientRequest, ConfigWarningNotification, RequestId, ServerNotification, ServerRequest,
    ThreadStartParams, ThreadStartResponse, TurnStartParams, TurnStartResponse, UserInput,
};
use peregrine_config::{CloudRequirementsLoader, LoaderOverrides};
use peregrine_utils_home_dir::find_peregrine_home;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{Mutex, oneshot};

type SessionMap = HashMap<String, DesktopAppServerSession>;

#[derive(Default)]
pub(crate) struct AgentServerSessions {
    sessions: Mutex<SessionMap>,
}

pub(crate) struct DesktopAppServerSession {
    request_handle: AppServerRequestHandle,
    thread_id: Option<String>,
    active_turn_id: Option<String>,
    turn_in_progress: bool,
    cwd: Option<PathBuf>,
    workspace_roots: Vec<PathBuf>,
    next_request_id: i64,
    pending_requests: HashMap<RequestId, ServerRequest>,
    stop_tx: Option<oneshot::Sender<()>>,
    event_task: tokio::task::JoinHandle<()>,
}

impl AgentServerSessions {
    pub(crate) async fn insert(
        &self,
        session_id: String,
        session: DesktopAppServerSession,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(&session_id) {
            return Err(format!(
                "agent app-server session `{session_id}` already exists"
            ));
        }
        sessions.insert(session_id, session);
        Ok(())
    }

    pub(crate) async fn set_thread(&self, session_id: &str, thread_id: String) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.thread_id = Some(thread_id);
        }
    }

    pub(crate) async fn set_active_turn(&self, session_id: &str, turn_id: String) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.active_turn_id = Some(turn_id);
            session.turn_in_progress = true;
        }
    }

    pub(crate) async fn apply_notification(
        &self,
        session_id: &str,
        notification: &ServerNotification,
    ) {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(session_id) else {
            return;
        };

        match notification {
            ServerNotification::TurnStarted(notification) => {
                session.thread_id = Some(notification.thread_id.clone());
                session.active_turn_id = Some(notification.turn.id.clone());
                session.turn_in_progress = true;
            }
            ServerNotification::TurnCompleted(notification) => {
                session.thread_id = Some(notification.thread_id.clone());
                session.active_turn_id = Some(notification.turn.id.clone());
                session.turn_in_progress = false;
            }
            ServerNotification::ServerRequestResolved(notification) => {
                session.pending_requests.remove(&notification.request_id);
            }
            _ => {}
        }
    }

    pub(crate) async fn add_pending_request(&self, session_id: &str, request: ServerRequest) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session
                .pending_requests
                .insert(request.id().clone(), request);
        }
    }

    pub(crate) async fn remove_pending_request(&self, session_id: &str, request_id: &RequestId) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.pending_requests.remove(request_id);
        }
    }

    pub(crate) async fn mark_disconnected(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(session_id);
    }

    pub(crate) async fn prepare_turn(
        &self,
        session_id: &str,
    ) -> Result<
        (
            AppServerRequestHandle,
            RequestId,
            String,
            Option<String>,
            Option<PathBuf>,
            Option<Vec<PathBuf>>,
        ),
        String,
    > {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("agent app-server session `{session_id}` was not found"))?;
        let thread_id = session
            .thread_id
            .clone()
            .ok_or_else(|| "agent app-server session has no thread yet".to_string())?;
        let active_turn_id = session
            .turn_in_progress
            .then(|| session.active_turn_id.clone())
            .flatten();
        let request_id = session.next_request_id();
        Ok((
            session.request_handle.clone(),
            request_id,
            thread_id,
            active_turn_id,
            session.cwd.clone(),
            Some(session.workspace_roots.clone()).filter(|roots| !roots.is_empty()),
        ))
    }

    pub(crate) async fn prepare_interrupt(
        &self,
        session_id: &str,
        turn_id: Option<String>,
    ) -> Result<(AppServerRequestHandle, RequestId, String, String), String> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("agent app-server session `{session_id}` was not found"))?;
        let thread_id = session
            .thread_id
            .clone()
            .ok_or_else(|| "agent app-server session has no thread yet".to_string())?;
        let turn_id = turn_id
            .or_else(|| session.active_turn_id.clone())
            .ok_or_else(|| {
                "agent app-server session has no active turn to interrupt".to_string()
            })?;
        let request_id = session.next_request_id();
        Ok((
            session.request_handle.clone(),
            request_id,
            thread_id,
            turn_id,
        ))
    }

    pub(crate) async fn prepare_request_resolution(
        &self,
        session_id: &str,
        request_id: &RequestId,
    ) -> Result<(AppServerRequestHandle, ServerRequest), String> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("agent app-server session `{session_id}` was not found"))?;
        let request = session
            .pending_requests
            .get(request_id)
            .cloned()
            .ok_or_else(|| format!("server request `{request_id}` was not found"))?;
        Ok((session.request_handle.clone(), request))
    }

    pub(crate) async fn stop(&self, session_id: &str) -> Result<(), String> {
        let session = {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(session_id)
        };
        let Some(mut session) = session else {
            return Ok(());
        };
        if let Some(stop_tx) = session.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        let _ = session.event_task.await;
        Ok(())
    }
}

impl DesktopAppServerSession {
    fn new(
        request_handle: AppServerRequestHandle,
        cwd: Option<PathBuf>,
        workspace_roots: Vec<PathBuf>,
        stop_tx: oneshot::Sender<()>,
        event_task: tokio::task::JoinHandle<()>,
    ) -> Self {
        Self {
            request_handle,
            thread_id: None,
            active_turn_id: None,
            turn_in_progress: false,
            cwd,
            workspace_roots,
            next_request_id: 1,
            pending_requests: HashMap::new(),
            stop_tx: Some(stop_tx),
            event_task,
        }
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId::Integer(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}

pub(crate) async fn start_agent_session(
    app: AppHandle,
    sessions: Arc<AgentServerSessions>,
    request: AgentServerStartRequest,
) -> Result<AgentServerStartResponse, String> {
    if request.session_id.trim().is_empty() {
        return Err("agent app-server session id cannot be empty".to_string());
    }

    let AgentServerStartRequest {
        session_id,
        agent_name,
        agent_role,
        agent_instructions,
        workflow_name,
        prompt,
        cwd,
        workspace_roots,
        target,
    } = request;
    let (client, target) =
        create_app_server_client(target, cwd.clone(), workspace_roots.clone()).await?;
    let request_handle = client.request_handle();
    let (stop_tx, stop_rx) = oneshot::channel();
    let event_task =
        events::spawn_event_pump(app, sessions.clone(), session_id.clone(), client, stop_rx);
    let session = DesktopAppServerSession::new(
        request_handle.clone(),
        if target.uses_remote_workspace() {
            None
        } else {
            cwd.clone()
        },
        workspace_roots.clone(),
        stop_tx,
        event_task,
    );
    sessions.insert(session_id.clone(), session).await?;

    let thread_request_id = next_request_id(&sessions, &session_id).await?;
    let thread_response: ThreadStartResponse = request_handle
        .request_typed(ClientRequest::ThreadStart {
            request_id: thread_request_id,
            params: ThreadStartParams {
                cwd: if target.uses_remote_workspace() {
                    None
                } else {
                    cwd.as_ref().map(|path| path.to_string_lossy().to_string())
                },
                runtime_workspace_roots: Some(workspace_roots.clone())
                    .filter(|roots| !roots.is_empty()),
                developer_instructions: Some(compose_developer_instructions(
                    &agent_name,
                    agent_role.as_deref(),
                    &workflow_name,
                    &agent_instructions,
                )),
                dynamic_tools: None,
                ..Default::default()
            },
        })
        .await
        .map_err(|err| err.to_string())?;
    let thread_id = thread_response.thread.id.clone();
    sessions.set_thread(&session_id, thread_id.clone()).await;

    let turn_request_id = next_request_id(&sessions, &session_id).await?;
    let turn_response: TurnStartResponse = request_handle
        .request_typed(ClientRequest::TurnStart {
            request_id: turn_request_id,
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                cwd: if target.uses_remote_workspace() {
                    None
                } else {
                    cwd
                },
                runtime_workspace_roots: Some(workspace_roots).filter(|roots| !roots.is_empty()),
                ..Default::default()
            },
        })
        .await
        .map_err(|err| err.to_string())?;
    let turn_id = turn_response.turn.id.clone();
    sessions.set_active_turn(&session_id, turn_id.clone()).await;

    Ok(AgentServerStartResponse {
        session_id,
        thread_id,
        thread: thread_response.thread,
        turn_id,
        model: thread_response.model,
        model_provider: thread_response.model_provider,
    })
}

pub(crate) async fn create_app_server_client(
    target_config: AgentServerTargetConfig,
    cwd: Option<PathBuf>,
    workspace_roots: Vec<PathBuf>,
) -> Result<(AppServerClient, ResolvedAgentServerTarget), String> {
    let target = resolve_target(target_config)?;
    let client = match &target {
        ResolvedAgentServerTarget::Embedded => {
            AppServerClient::InProcess(start_embedded_client(cwd, workspace_roots).await?)
        }
        ResolvedAgentServerTarget::LocalDaemon { endpoint }
        | ResolvedAgentServerTarget::Remote { endpoint } => AppServerClient::Remote(
            RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
                endpoint: endpoint.clone(),
                client_name: "peregrine-desktop".to_string(),
                client_version: env!("CARGO_PKG_VERSION").to_string(),
                experimental_api: true,
                opt_out_notification_methods: Vec::new(),
                channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
            })
            .await
            .map_err(|err| err.to_string())?,
        ),
    };
    Ok((client, target))
}

async fn start_embedded_client(
    cwd: Option<PathBuf>,
    workspace_roots: Vec<PathBuf>,
) -> Result<InProcessAppServerClient, String> {
    let cloud_requirements = CloudRequirementsLoader::default();
    let loader_overrides = LoaderOverrides::default();
    let config = peregrine_app_server_client::legacy_core::config::ConfigBuilder::default()
        .harness_overrides(
            peregrine_app_server_client::legacy_core::config::ConfigOverrides {
                cwd: cwd.clone(),
                workspace_roots: Some(workspace_roots.clone()).filter(|roots| !roots.is_empty()),
                ..Default::default()
            },
        )
        .loader_overrides(loader_overrides.clone())
        .cloud_requirements(cloud_requirements.clone())
        .strict_config(false)
        .build()
        .await
        .map_err(|err| err.to_string())?;

    let peregrine_home =
        find_peregrine_home().map_err(|err| format!("failed to resolve PEREGRINE_HOME: {err}"))?;
    let local_runtime_paths =
        ExecServerRuntimePaths::from_optional_paths(None, None).map_err(|err| err.to_string())?;
    let environment_manager =
        EnvironmentManager::from_codex_home(peregrine_home, Some(local_runtime_paths))
            .await
            .map(Arc::new)
            .map_err(|err| err.to_string())?;
    let state_db = state_db::try_init(&config)
        .await
        .map(Some)
        .map_err(|err| err.to_string())?;
    let config_warnings = config
        .startup_warnings
        .iter()
        .map(|warning| ConfigWarningNotification {
            summary: warning.clone(),
            details: None,
            path: None,
            range: None,
        })
        .collect();

    InProcessAppServerClient::start(InProcessClientStartArgs {
        arg0_paths: Arg0DispatchPaths {
            codex_self_exe: crate::helper_args::resolve_helper_executable().ok(),
            ..Default::default()
        },
        config: Arc::new(config),
        cli_overrides: Vec::<(String, toml::Value)>::new(),
        loader_overrides,
        strict_config: false,
        cloud_requirements,
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db,
        environment_manager,
        config_warnings,
        session_source: serde_json::from_value(json!({"custom": "desktop"}))
            .map_err(|err| err.to_string())?,
        enable_peregrine_api_key_env: false,
        client_name: "peregrine-desktop".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await
    .map_err(|err| err.to_string())
}

async fn next_request_id(
    sessions: &AgentServerSessions,
    session_id: &str,
) -> Result<RequestId, String> {
    let mut sessions_guard = sessions.sessions.lock().await;
    let session = sessions_guard
        .get_mut(session_id)
        .ok_or_else(|| format!("agent app-server session `{session_id}` was not found"))?;
    Ok(session.next_request_id())
}

fn compose_developer_instructions(
    agent_name: &str,
    agent_role: Option<&str>,
    workflow_name: &str,
    agent_instructions: &str,
) -> String {
    let role_line = agent_role
        .map(|role| format!("\nApp-server role: {role}"))
        .unwrap_or_default();
    format!(
        "Desktop agent: {agent_name}{role_line}\nWorkflow: {workflow_name}\n\n{agent_instructions}"
    )
}
