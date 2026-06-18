use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use chrono::{DateTime, Local};
use codex_exec_server::{EnvironmentManager, ExecServerRuntimePaths};
use codex_otel::SessionTelemetry;
use codex_terminal_detection::user_agent;
use codex_utils_absolute_path::AbsolutePathBuf;
use color_eyre::eyre::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use peregrine_app_server_client::AppServerEvent;
use peregrine_app_server_protocol::{
    ApprovalsReviewer as AppServerApprovalsReviewer, ClientRequest,
    CommandExecutionRequestApprovalResponse, FileChangeRequestApprovalResponse,
    GetAccountRateLimitsResponse, McpServerElicitationRequestResponse, McpServerStatus,
    McpServerStatusDetail, ModelProviderListParams, ModelProviderListResponse,
    ModelProviderModelsListParams, ModelProviderModelsListResponse, ModelProviderSelectParams,
    ModelProviderSelectResponse, PermissionsRequestApprovalResponse, RateLimitSnapshot, RequestId,
    RequestId as AppServerRequestId, ServerNotification, ServerRequest, SkillsListParams,
    SkillsListResponse, Thread, ThreadGoal, ThreadGoalClearResponse, ThreadGoalGetResponse,
    ThreadGoalSetResponse, ThreadGoalStatus, ThreadListCwdFilter, ThreadListParams,
    ThreadSettingsUpdateParams, ThreadSortKey, TurnStatus,
};
use peregrine_config::{CloudRequirementsLoader, LoaderOverrides};
use peregrine_types::ThreadId;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::agent::AppServerTarget;
use crate::agent::app_command::AppCommand;
use crate::agent::app_event::{
    AppEvent, HistoryLookupResponse, RateLimitRefreshOrigin, ThreadGoalSetMode,
};
use crate::agent::app_event_sender::AppEventSender;
use crate::agent::app_server_approval_conversions::granted_permission_profile_from_request;
use crate::agent::app_server_session::{
    AppServerBootstrap, AppServerSession, AppServerStartedThread, TurnPermissionsOverride,
    app_server_rate_limit_snapshots,
};
use crate::agent::audit_command::{AuditCommand, AuditCommandOutput, execute_audit_command};
use crate::agent::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::agent::bottom_pane::{SelectionAction, SelectionItem, SelectionViewParams};
use crate::agent::chatwidget::{
    ChatWidget, ChatWidgetInit, ReplayKind, create_initial_user_message,
};
use crate::agent::goal_display::{goal_status_label, goal_usage_summary};
use crate::agent::history_cell::HistoryCell;
use crate::agent::legacy_core::config::{Config, ConfigBuilder, ConfigOverrides};
use crate::agent::model_catalog::ModelCatalog;
use crate::agent::resume_source_kinds;
use crate::agent::status::StatusAccountDisplay;
use crate::agent::tui::FrameRequester;
use crate::theme::ThemePalette;
use uuid::Uuid;

mod mcp_inventory;
mod navigation;
mod provider;
#[cfg(test)]
mod tests;

const SESSION_PAGE_SIZE: u32 = 25;
const APP_SERVER_EVENT_DRAIN_LIMIT: usize = 32;
const APP_EVENT_DRAIN_LIMIT: usize = 128;
const EPHEMERAL_THREAD_GOAL_ERROR_MESSAGE: &str = concat!(
    "Goals need a saved session. This session is temporary.\n",
    "Run `peregrine agent` to start a saved session, or `peregrine resume` / `/resume` to reopen one.",
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChatAction {
    None,
    FocusCode,
    Quit,
    ThemeSelected(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostMode {
    Idle,
    Loading,
    Sessions,
    Chat,
    Error,
}

#[derive(Debug, Clone)]
struct SessionRow {
    thread_id: ThreadId,
    title: String,
    cwd: PathBuf,
    updated_at: i64,
}

#[derive(Clone)]
struct ChatContext {
    config: Config,
    model_catalog: Arc<ModelCatalog>,
    app_event_tx: AppEventSender,
    has_chatgpt_account: bool,
    status_account_display: Option<StatusAccountDisplay>,
    initial_plan_type: Option<peregrine_types::account::PlanType>,
    model: String,
    account_email: Option<String>,
    auth_mode: Option<codex_otel::TelemetryAuthMode>,
    status_line_invalid_items_warned: Arc<AtomicBool>,
    terminal_title_invalid_items_warned: Arc<AtomicBool>,
}

#[derive(Default)]
struct PendingRequests {
    exec_approvals: HashMap<String, AppServerRequestId>,
    file_change_approvals: HashMap<String, AppServerRequestId>,
    permission_approvals: HashMap<String, AppServerRequestId>,
    user_inputs: HashMap<String, VecDeque<PendingUserInputRequest>>,
    mcp_requests: Vec<PendingMcpRequest>,
}

struct PendingUserInputRequest {
    _item_id: String,
    request_id: AppServerRequestId,
}

struct PendingMcpRequest {
    server_name: String,
    request_id: AppServerRequestId,
}

enum WorkerCommand {
    Shutdown,
    Suspend {
        response_tx: std::sync::mpsc::SyncSender<AppServerSession>,
    },
    StartFresh {
        initial_text: Option<String>,
        replace_widget: bool,
    },
    Resume {
        thread_id: ThreadId,
    },
    ReloadSessions,
    ResumeByIdOrName(String),
    SubmitOp {
        thread_id: ThreadId,
        active_turn_id: Option<String>,
        op: AppCommand,
    },
    ResolveRequest {
        request_id: AppServerRequestId,
        result: serde_json::Value,
    },
    ListSkills {
        cwds: Vec<PathBuf>,
        force_reload: bool,
    },
    SyncSettings(ThreadSettingsUpdateParams),
    RefreshRateLimits {
        origin: RateLimitRefreshOrigin,
    },
    FetchMcpInventory {
        detail: McpServerStatusDetail,
        thread_id: Option<ThreadId>,
    },
    LoadProviders,
    LoadProviderModels {
        provider_id: String,
        provider_display_name: String,
    },
    SelectProvider {
        provider_id: String,
        model: Option<String>,
    },
    OpenGoalMenu {
        thread_id: ThreadId,
    },
    OpenGoalEditor {
        thread_id: Option<ThreadId>,
    },
    SetGoalObjective {
        thread_id: ThreadId,
        objective: String,
        mode: ThreadGoalSetMode,
    },
    SetGoalStatus {
        thread_id: ThreadId,
        status: ThreadGoalStatus,
    },
    ClearGoal {
        thread_id: ThreadId,
    },
    RunAuditCommand {
        command: AuditCommand,
        command_text: String,
    },
    SetSkillEnabled {
        path: AbsolutePathBuf,
        enabled: bool,
    },
    SelectSyntaxTheme {
        name: String,
    },
}

enum WorkerEvent {
    Started {
        command_tx: UnboundedSender<WorkerCommand>,
        context: ChatContext,
        sessions: Vec<SessionRow>,
    },
    ConnectionResumed {
        command_tx: UnboundedSender<WorkerCommand>,
    },
    StartupFailed(String),
    Stopped(String),
    AppServer(AppServerEvent),
    FreshStarted {
        initial_text: Option<String>,
        replace_widget: bool,
        result: std::result::Result<AppServerStartedThread, String>,
    },
    Resumed {
        result: std::result::Result<AppServerStartedThread, String>,
    },
    SessionsLoaded(std::result::Result<Vec<SessionRow>, String>),
    SessionLookup {
        id_or_name: String,
        result: std::result::Result<Option<SessionRow>, String>,
    },
    OpSubmitted(std::result::Result<(), String>),
    RequestResolved(std::result::Result<(), String>),
    SkillsListed(std::result::Result<SkillsListResponse, String>),
    SettingsSynced(std::result::Result<(), String>),
    RateLimitsLoaded {
        origin: RateLimitRefreshOrigin,
        result: std::result::Result<Vec<RateLimitSnapshot>, String>,
    },
    McpInventoryLoaded {
        detail: McpServerStatusDetail,
        thread_id: Option<ThreadId>,
        result: std::result::Result<Vec<McpServerStatus>, String>,
    },
    ProvidersLoaded(std::result::Result<ModelProviderListResponse, String>),
    ProviderModelsLoaded {
        provider_id: String,
        provider_display_name: String,
        result: std::result::Result<ModelProviderModelsListResponse, String>,
    },
    ProviderSelected(std::result::Result<ModelProviderSelectResponse, String>),
    GoalMenu {
        thread_id: ThreadId,
        result: std::result::Result<ThreadGoalGetResponse, String>,
    },
    GoalEditor {
        thread_id: Option<ThreadId>,
        result: std::result::Result<ThreadGoalGetResponse, String>,
    },
    GoalReplaceConfirmation {
        thread_id: ThreadId,
        objective: String,
    },
    GoalObjectiveSet {
        thread_id: ThreadId,
        replacing_goal: bool,
        result: std::result::Result<ThreadGoalSetResponse, String>,
    },
    GoalStatusSet {
        thread_id: ThreadId,
        result: std::result::Result<ThreadGoalSetResponse, String>,
    },
    GoalCleared {
        thread_id: ThreadId,
        result: std::result::Result<ThreadGoalClearResponse, String>,
    },
    AuditCommandFinished {
        command_text: String,
        result: std::result::Result<AuditCommandOutput, String>,
    },
    SkillEnabledSet {
        path: AbsolutePathBuf,
        enabled: bool,
        result: std::result::Result<(), String>,
    },
    SyntaxThemeSelected {
        name: String,
        result: std::result::Result<(), String>,
    },
}

pub(crate) struct ChatController {
    initial_config: Option<Config>,
    application_runtime: Option<crate::app::ApplicationRuntime>,
    runtime: Option<Arc<tokio::runtime::Runtime>>,
    mode: HostMode,
    status: String,
    worker_tx: Option<UnboundedSender<WorkerCommand>>,
    worker_event_rx: Option<UnboundedReceiver<WorkerEvent>>,
    context: Option<ChatContext>,
    app_event_rx: Option<UnboundedReceiver<AppEvent>>,
    chat_widget: Option<ChatWidget>,
    history_cells: Vec<Arc<dyn HistoryCell>>,
    sessions: Vec<SessionRow>,
    selected_session: usize,
    session_list_offset: usize,
    active_thread_id: Option<ThreadId>,
    active_turn_id: Option<String>,
    session_request_pending: bool,
    handoff_thread_id: Option<ThreadId>,
    pending_requests: PendingRequests,
    transcript_scroll: usize,
}

impl Default for ChatController {
    fn default() -> Self {
        Self {
            initial_config: None,
            application_runtime: None,
            runtime: None,
            mode: HostMode::Idle,
            status: "chat: idle".to_string(),
            worker_tx: None,
            worker_event_rx: None,
            context: None,
            app_event_rx: None,
            chat_widget: None,
            history_cells: Vec::new(),
            sessions: Vec::new(),
            selected_session: 0,
            session_list_offset: 0,
            active_thread_id: None,
            active_turn_id: None,
            session_request_pending: false,
            handoff_thread_id: None,
            pending_requests: PendingRequests::default(),
            transcript_scroll: 0,
        }
    }
}

impl ChatController {
    pub(crate) fn new(config: Config, application_runtime: crate::app::ApplicationRuntime) -> Self {
        let mut controller = Self::default();
        controller.initial_config = Some(config);
        controller.application_runtime = Some(application_runtime);
        controller
    }

    pub(crate) fn active_thread_id(&self) -> Option<ThreadId> {
        self.active_thread_id
    }

    pub(crate) fn adopt_thread(&mut self, root: &Path, thread_id: ThreadId) {
        if self.active_thread_id.as_ref() == Some(&thread_id) {
            return;
        }
        self.handoff_thread_id = Some(thread_id);
        self.ensure_started(root);
        if self.worker_tx.is_some() {
            self.handoff_thread_id = None;
            let _ = self.resume_session(thread_id);
        }
    }

    pub(crate) fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn tick(&mut self, root: &Path) -> ChatAction {
        self.ensure_started(root);
        let mut action = self.drain_worker_events();
        if !matches!(self.mode, HostMode::Chat | HostMode::Sessions) {
            return action;
        }

        if let Some(chat) = self.chat_widget.as_mut() {
            let _guard = self.runtime.as_ref().map(|runtime| runtime.enter());
            chat.flush_paste_burst_if_due();
            chat.pre_draw_tick();
        }

        action = combine_action(action, self.drain_app_events());
        if self
            .chat_widget
            .as_ref()
            .is_some_and(ChatWidget::has_queued_follow_up_messages)
            && self.active_thread_id.is_none()
        {
            action = combine_action(action, self.start_fresh_session(None, false));
        }
        action = combine_action(action, self.drain_app_events());
        action
    }

    pub(crate) fn handle_key(&mut self, root: &Path, key: KeyEvent) -> ChatAction {
        self.ensure_started(root);
        if self.should_open_session_history(key) {
            self.open_session_history();
            return ChatAction::None;
        }
        let mut action = ChatAction::None;
        if matches!(self.mode, HostMode::Sessions) {
            match session_list_key_action(key) {
                SessionListKeyAction::Previous => {
                    self.select_previous_session();
                    return ChatAction::None;
                }
                SessionListKeyAction::Next => {
                    self.select_next_session();
                    return ChatAction::None;
                }
                SessionListKeyAction::ResumeSelected => {
                    return self.resume_selected_session();
                }
                SessionListKeyAction::StartFresh => {
                    self.mode = HostMode::Chat;
                    self.status = "chat: new session".to_string();
                    return ChatAction::None;
                }
                SessionListKeyAction::PassToComposer => {
                    self.mode = HostMode::Chat;
                }
            }
        }

        if let Some(chat) = self.chat_widget.as_mut() {
            let _guard = self.runtime.as_ref().map(|runtime| runtime.enter());
            chat.handle_key_event(key);
        }
        if self
            .chat_widget
            .as_ref()
            .is_some_and(ChatWidget::has_queued_follow_up_messages)
            && self.active_thread_id.is_none()
        {
            action = combine_action(action, self.start_fresh_session(None, false));
        }
        combine_action(action, self.drain_app_events())
    }

    pub(crate) fn handle_paste(&mut self, root: &Path, text: String) -> ChatAction {
        self.ensure_started(root);
        if matches!(self.mode, HostMode::Sessions) {
            self.mode = HostMode::Chat;
        }
        if let Some(chat) = self.chat_widget.as_mut() {
            let _guard = self.runtime.as_ref().map(|runtime| runtime.enter());
            chat.handle_paste(text);
        }
        self.drain_app_events()
    }

    pub(crate) fn scroll(&mut self, direction: crate::ScrollDirection, amount: usize) {
        match self.mode {
            HostMode::Sessions => match direction {
                crate::ScrollDirection::Up => {
                    for _ in 0..amount {
                        self.select_previous_session();
                    }
                }
                crate::ScrollDirection::Down => {
                    for _ in 0..amount {
                        self.select_next_session();
                    }
                }
                crate::ScrollDirection::Left | crate::ScrollDirection::Right => {}
            },
            HostMode::Chat => match direction {
                crate::ScrollDirection::Up => {
                    self.transcript_scroll = self.transcript_scroll.saturating_add(amount);
                }
                crate::ScrollDirection::Down => {
                    self.transcript_scroll = self.transcript_scroll.saturating_sub(amount);
                }
                crate::ScrollDirection::Left | crate::ScrollDirection::Right => {}
            },
            HostMode::Idle | HostMode::Loading | HostMode::Error => {}
        }
    }

    pub(crate) fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        frame.buffer_mut().set_style(area, chat_base_style(palette));
        match self.mode {
            HostMode::Idle | HostMode::Loading | HostMode::Error => {
                self.render_message(frame, area, focused, palette);
            }
            HostMode::Sessions => self.render_sessions(frame, area, focused, palette),
            HostMode::Chat => self.render_chat(frame, area, focused, palette),
        }
    }

    pub(crate) fn shutdown(&mut self) {
        let Some(runtime) = self.runtime.take() else {
            return;
        };
        if let Some(worker_tx) = self.worker_tx.take() {
            let _ = worker_tx.send(WorkerCommand::Shutdown);
        }
        shutdown_owned_runtime(runtime);
    }

    pub(crate) fn suspend(&mut self) -> std::io::Result<()> {
        let Some(runtime) = self.runtime.take() else {
            return Ok(());
        };
        let Some(worker_tx) = self.worker_tx.take() else {
            shutdown_owned_runtime(runtime);
            return Ok(());
        };
        let (response_tx, response_rx) = std::sync::mpsc::sync_channel(1);
        worker_tx
            .send(WorkerCommand::Suspend { response_tx })
            .map_err(|_| std::io::Error::other("workbench chat worker is not running"))?;
        let app_server = response_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| std::io::Error::other("timed out suspending workbench chat"))?;
        self.worker_event_rx = None;
        if let Some(application_runtime) = &self.application_runtime {
            application_runtime.store_app_server(app_server);
        }
        shutdown_owned_runtime(runtime);
        Ok(())
    }

    fn ensure_started(&mut self, root: &Path) {
        if self.runtime.is_some() {
            return;
        }

        let preserve_ui_state = self.context.is_some();
        if !preserve_ui_state {
            self.mode = HostMode::Loading;
            self.status = "chat: loading sessions".to_string();
        }
        let runtime = match self.application_runtime.as_ref() {
            Some(application_runtime) => application_runtime.async_runtime(),
            None => match crate::build_agent_runtime() {
                Ok(runtime) => Arc::new(runtime),
                Err(err) => {
                    self.mode = HostMode::Error;
                    self.status = format!("chat: runtime failed: {err}");
                    return;
                }
            },
        };
        let (worker_event_tx, worker_event_rx) = unbounded_channel();
        let app_event_tx = match self.context.as_ref() {
            Some(context) => context.app_event_tx.clone(),
            None => {
                let (app_event_tx, app_event_rx) = unbounded_channel();
                self.app_event_rx = Some(app_event_rx);
                AppEventSender::new(app_event_tx)
            }
        };
        let root = root.to_path_buf();
        let initial_config = self.initial_config.clone();
        let app_server = self
            .application_runtime
            .as_ref()
            .and_then(crate::app::ApplicationRuntime::take_app_server);
        runtime.spawn(async move {
            start_worker(
                root,
                initial_config,
                app_server,
                preserve_ui_state,
                app_event_tx,
                worker_event_tx,
            )
            .await;
        });
        self.worker_event_rx = Some(worker_event_rx);
        self.runtime = Some(runtime);
    }

    fn render_message(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        let body = match self.mode {
            HostMode::Idle => "Open chat to load previous sessions.",
            HostMode::Loading => "Loading previous chat sessions...",
            HostMode::Error => self.status.as_str(),
            HostMode::Sessions | HostMode::Chat => "",
        };
        let paragraph = Paragraph::new(body)
            .style(chat_muted_style(palette))
            .block(chat_block(self.chat_title(), focused, palette))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_sessions(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        let rows = self.chat_rows(area);
        self.render_session_list(frame, rows[0], focused, palette);
        self.render_composer(frame, rows[1], focused, palette);
    }

    fn render_session_list(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        if self.sessions.is_empty() {
            let lines = vec![
                Line::from("No previous chats for this project."),
                Line::from(""),
                Line::styled(
                    "Type a prompt below to start a fresh chat.",
                    chat_muted_style(palette),
                ),
            ];
            let paragraph = Paragraph::new(lines)
                .style(chat_base_style(palette))
                .block(chat_block(self.chat_title(), focused, palette));
            frame.render_widget(paragraph, area);
            return;
        }

        let items = self
            .sessions
            .iter()
            .map(|session| {
                ListItem::new(vec![
                    Line::from(session.title.clone()),
                    Line::from(vec![
                        Span::styled(
                            format_updated_at(session.updated_at),
                            chat_muted_style(palette),
                        ),
                        Span::raw("  "),
                        Span::styled(session.cwd.display().to_string(), chat_base_style(palette)),
                    ]),
                ])
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(self.selected_session));
        let list = List::new(items)
            .block(chat_block(self.chat_title(), focused, palette))
            .style(chat_base_style(palette))
            .highlight_symbol("> ")
            .highlight_style(chat_selection_style(palette));
        frame.render_stateful_widget(list, area, &mut state);
        self.session_list_offset = state.offset();
    }

    fn render_chat(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        let rows = self.chat_rows(area);
        self.render_transcript(frame, rows[0], focused, palette);
        self.render_composer_popup_overlay(frame, rows[0], palette);
        self.render_composer(frame, rows[1], focused, palette);
    }

    fn render_transcript(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        let Some(chat) = self.chat_widget.as_ref() else {
            self.render_message(frame, area, focused, palette);
            return;
        };
        let lines = chat.embedded_transcript_lines(&self.history_cells, area.width);
        let visible_height = area.height.saturating_sub(2) as usize;
        let max_scroll = lines.len().saturating_sub(visible_height);
        self.transcript_scroll = self.transcript_scroll.min(max_scroll);
        let scroll = max_scroll.saturating_sub(self.transcript_scroll);
        let paragraph = Paragraph::new(lines)
            .style(chat_base_style(palette))
            .block(chat_block(self.chat_title(), focused, palette))
            .scroll((u16_saturating(scroll), 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn chat_title(&self) -> String {
        self.chat_widget
            .as_ref()
            .map(|chat| chat.current_model().to_string())
            .or_else(|| self.context.as_ref().map(|context| context.model.clone()))
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| "model".to_string())
    }

    fn render_composer(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        focused: bool,
        palette: ThemePalette,
    ) {
        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        let _guard = self.runtime.as_ref().map(|runtime| runtime.enter());
        frame.buffer_mut().set_style(area, chat_base_style(palette));
        chat.render_embedded_bottom_with_popup_overlay(area, frame.buffer_mut());
        if focused && let Some((x, y)) = chat.embedded_bottom_cursor_pos_with_popup_overlay(area) {
            frame.set_cursor_position(Position::new(x, y));
        }
    }

    fn render_composer_popup_overlay(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        palette: ThemePalette,
    ) {
        let Some(chat) = self.chat_widget.as_ref() else {
            return;
        };
        let _guard = self.runtime.as_ref().map(|runtime| runtime.enter());
        let body = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        if body.is_empty() {
            return;
        }
        let Some(height) = chat.embedded_bottom_popup_overlay_height(body.width) else {
            return;
        };
        let height = height.min(body.height);
        if height == 0 {
            return;
        }
        let popup_area = Rect {
            x: body.x,
            y: body.y.saturating_add(body.height.saturating_sub(height)),
            width: body.width,
            height,
        };
        frame.render_widget(Clear, popup_area);
        frame
            .buffer_mut()
            .set_style(popup_area, chat_base_style(palette));
        chat.render_embedded_bottom_popup_overlay(popup_area, frame.buffer_mut());
    }

    fn chat_rows(&self, area: Rect) -> Vec<Rect> {
        let bottom_height = self
            .chat_widget
            .as_ref()
            .map(|chat| chat.embedded_bottom_height_with_popup_overlay(area.width))
            .unwrap_or(3)
            .max(1)
            .min(area.height.saturating_sub(1).max(1));
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(bottom_height)])
            .split(area)
            .to_vec()
    }

    fn select_previous_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        self.selected_session = self.selected_session.saturating_sub(1);
    }

    fn select_next_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        self.selected_session = (self.selected_session + 1).min(self.sessions.len() - 1);
    }

    fn resume_selected_session(&mut self) -> ChatAction {
        let Some(session) = self.sessions.get(self.selected_session).cloned() else {
            self.mode = HostMode::Chat;
            self.status = "chat: new session".to_string();
            return ChatAction::None;
        };
        self.resume_session(session.thread_id)
    }

    fn resume_session(&mut self, thread_id: ThreadId) -> ChatAction {
        if self.session_request_pending {
            return ChatAction::None;
        }
        self.status = "chat: resuming session".to_string();
        if self.send_worker(WorkerCommand::Resume { thread_id }) {
            self.session_request_pending = true;
        } else {
            self.status = "chat: worker is not ready".to_string();
        }
        ChatAction::None
    }

    fn start_fresh_session(
        &mut self,
        initial_text: Option<String>,
        replace_widget: bool,
    ) -> ChatAction {
        if self.session_request_pending {
            return ChatAction::None;
        }
        self.status = "chat: starting session".to_string();
        if self.send_worker(WorkerCommand::StartFresh {
            initial_text,
            replace_widget,
        }) {
            self.session_request_pending = true;
        } else {
            self.status = "chat: worker is not ready".to_string();
        }
        ChatAction::None
    }

    fn attach_thread(
        &mut self,
        started: AppServerStartedThread,
        replace_widget: bool,
        initial_text: Option<String>,
    ) {
        self.session_request_pending = false;
        self.active_thread_id = Some(started.session.thread_id);
        self.active_turn_id = started
            .turns
            .iter()
            .find(|turn| matches!(turn.status, TurnStatus::InProgress))
            .map(|turn| turn.id.clone());
        if replace_widget {
            self.history_cells.clear();
            self.pending_requests = PendingRequests::default();
            self.chat_widget = self.build_chat_widget(initial_text);
        }
        if let Some(chat) = self.chat_widget.as_mut() {
            chat.set_queue_submissions_until_session_configured(false);
            chat.handle_thread_session(started.session);
            chat.replay_thread_turns(started.turns, ReplayKind::ResumeInitialMessages);
        }
        self.transcript_scroll = 0;
    }

    fn build_chat_widget(&self, initial_text: Option<String>) -> Option<ChatWidget> {
        let runtime = self.runtime.as_ref()?;
        let _guard = runtime.enter();
        let context = self.context.as_ref()?;
        let initial_user_message =
            create_initial_user_message(initial_text, Vec::new(), Vec::new());
        let (draw_tx, _draw_rx) = broadcast::channel(8);
        let frame_requester = FrameRequester::new(draw_tx);
        let mut chat = ChatWidget::new_with_app_event(ChatWidgetInit {
            config: context.config.clone(),
            frame_requester,
            app_event_tx: context.app_event_tx.clone(),
            workspace_command_runner: None,
            initial_user_message,
            enhanced_keys_supported: false,
            has_chatgpt_account: context.has_chatgpt_account,
            model_catalog: context.model_catalog.clone(),
            feedback: codex_feedback::CodexFeedback::new(),
            is_first_run: false,
            status_account_display: context.status_account_display.clone(),
            runtime_model_provider_base_url: None,
            initial_plan_type: context.initial_plan_type,
            model: Some(context.model.clone()),
            startup_tooltip_override: None,
            status_line_invalid_items_warned: context.status_line_invalid_items_warned.clone(),
            terminal_title_invalid_items_warned: context
                .terminal_title_invalid_items_warned
                .clone(),
            session_telemetry: new_session_telemetry(context),
        });
        chat.set_session_header_directory_visible(false);
        chat.set_status_line_visible(false);
        chat.set_queue_submissions_until_session_configured(true);
        chat.set_mcp_startup_expected_servers(enabled_mcp_server_names(&context.config));
        Some(chat)
    }

    fn refresh_mcp_startup_expected_servers(&mut self) {
        let Some(context) = self.context.as_ref() else {
            return;
        };
        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        chat.set_mcp_startup_expected_servers(enabled_mcp_server_names(&context.config));
    }

    fn send_worker(&self, command: WorkerCommand) -> bool {
        self.worker_tx
            .as_ref()
            .is_some_and(|worker_tx| worker_tx.send(command).is_ok())
    }

    fn drain_worker_events(&mut self) -> ChatAction {
        let mut action = ChatAction::None;
        for _ in 0..APP_SERVER_EVENT_DRAIN_LIMIT {
            let event = match self
                .worker_event_rx
                .as_mut()
                .and_then(|rx| rx.try_recv().ok())
            {
                Some(event) => event,
                None => break,
            };
            action = combine_action(action, self.handle_worker_event(event));
        }
        action
    }

    fn handle_worker_event(&mut self, event: WorkerEvent) -> ChatAction {
        match event {
            WorkerEvent::Started {
                command_tx,
                context,
                sessions,
            } => {
                self.worker_tx = Some(command_tx);
                self.context = Some(context);
                self.sessions = sessions;
                self.selected_session = self
                    .selected_session
                    .min(self.sessions.len().saturating_sub(1));
                self.active_thread_id = None;
                self.active_turn_id = None;
                self.session_request_pending = false;
                self.pending_requests = PendingRequests::default();
                self.history_cells.clear();
                self.chat_widget = self.build_chat_widget(None);
                self.mode = HostMode::Sessions;
                self.status = if self.sessions.is_empty() {
                    "chat: no previous sessions".to_string()
                } else {
                    format!("chat: {} previous sessions", self.sessions.len())
                };
                if let Some(thread_id) = self.handoff_thread_id.take() {
                    return self.resume_session(thread_id);
                }
                self.drain_app_events()
            }
            WorkerEvent::ConnectionResumed { command_tx } => {
                self.worker_tx = Some(command_tx);
                self.status = "chat: ready".to_string();
                if let Some(thread_id) = self.handoff_thread_id.take() {
                    return self.resume_session(thread_id);
                }
                self.drain_app_events()
            }
            WorkerEvent::StartupFailed(message) => {
                self.session_request_pending = false;
                self.mode = HostMode::Error;
                self.status = format!("chat: {message}");
                ChatAction::None
            }
            WorkerEvent::Stopped(message) => {
                self.session_request_pending = false;
                self.status = format!("chat: {message}");
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(self.status.clone());
                }
                self.drain_app_events()
            }
            WorkerEvent::AppServer(event) => self.handle_app_server_event(event),
            WorkerEvent::FreshStarted {
                initial_text,
                replace_widget,
                result,
            } => match result {
                Ok(started) => {
                    self.session_request_pending = false;
                    self.attach_thread(started, replace_widget, initial_text);
                    self.mode = HostMode::Chat;
                    self.status = "chat: ready".to_string();
                    if let Some(chat) = self.chat_widget.as_mut() {
                        chat.maybe_send_next_queued_input();
                    }
                    self.drain_app_events()
                }
                Err(message) => {
                    self.session_request_pending = false;
                    self.status = format!("chat: failed to start session: {message}");
                    if let Some(chat) = self.chat_widget.as_mut() {
                        chat.add_error_message(self.status.clone());
                    }
                    self.drain_app_events()
                }
            },
            WorkerEvent::Resumed { result } => match result {
                Ok(started) => {
                    self.session_request_pending = false;
                    self.attach_thread(started, true, None);
                    self.mode = HostMode::Chat;
                    self.status = "chat: ready".to_string();
                    self.drain_app_events()
                }
                Err(message) => {
                    self.session_request_pending = false;
                    self.status = format!("chat: resume failed: {message}");
                    if let Some(chat) = self.chat_widget.as_mut() {
                        chat.add_error_message(self.status.clone());
                    }
                    self.drain_app_events()
                }
            },
            WorkerEvent::SessionsLoaded(result) => {
                match result {
                    Ok(sessions) => {
                        self.sessions = sessions;
                        self.selected_session = self
                            .selected_session
                            .min(self.sessions.len().saturating_sub(1));
                        self.status = if self.sessions.is_empty() {
                            "chat: no previous sessions".to_string()
                        } else {
                            format!("chat: {} previous sessions", self.sessions.len())
                        };
                    }
                    Err(message) => {
                        self.status = format!("chat: session reload failed: {message}");
                        if let Some(chat) = self.chat_widget.as_mut() {
                            chat.add_error_message(self.status.clone());
                        }
                    }
                }
                self.drain_app_events()
            }
            WorkerEvent::SessionLookup { id_or_name, result } => match result {
                Ok(Some(row)) => self.resume_session(row.thread_id),
                Ok(None) => {
                    if let Some(chat) = self.chat_widget.as_mut() {
                        chat.add_error_message(format!(
                            "No saved chat found matching '{id_or_name}'."
                        ));
                    }
                    self.drain_app_events()
                }
                Err(message) => {
                    if let Some(chat) = self.chat_widget.as_mut() {
                        chat.add_error_message(format!("Session lookup failed: {message}"));
                    }
                    self.drain_app_events()
                }
            },
            WorkerEvent::OpSubmitted(result) => {
                if let Err(message) = result
                    && let Some(chat) = self.chat_widget.as_mut()
                {
                    chat.add_error_message(format!("chat command failed: {message}"));
                }
                self.drain_app_events()
            }
            WorkerEvent::RequestResolved(result) => {
                if let Err(message) = result
                    && let Some(chat) = self.chat_widget.as_mut()
                {
                    chat.add_error_message(format!(
                        "failed to resolve app-server request: {message}"
                    ));
                }
                self.drain_app_events()
            }
            WorkerEvent::SkillsListed(result) => {
                match result {
                    Ok(response) => {
                        if let Some(chat) = self.chat_widget.as_mut() {
                            chat.handle_skills_list_response(response);
                        }
                    }
                    Err(message) => {
                        if let Some(chat) = self.chat_widget.as_mut() {
                            chat.add_error_message(format!("failed to refresh skills: {message}"));
                        }
                    }
                }
                self.drain_app_events()
            }
            WorkerEvent::SettingsSynced(result) => {
                if let Err(message) = result
                    && let Some(chat) = self.chat_widget.as_mut()
                {
                    chat.add_error_message(format!("Failed to update thread settings: {message}"));
                }
                self.drain_app_events()
            }
            WorkerEvent::RateLimitsLoaded { origin, result } => {
                self.apply_rate_limit_result(origin, result);
                self.drain_app_events()
            }
            WorkerEvent::McpInventoryLoaded {
                detail,
                thread_id,
                result,
            } => {
                self.apply_mcp_inventory_result(result, detail, thread_id);
                self.drain_app_events()
            }
            WorkerEvent::ProvidersLoaded(result) => {
                self.apply_provider_list_result(result);
                self.drain_app_events()
            }
            WorkerEvent::ProviderModelsLoaded {
                provider_id,
                provider_display_name,
                result,
            } => {
                self.apply_provider_models_result(provider_id, provider_display_name, result);
                self.drain_app_events()
            }
            WorkerEvent::ProviderSelected(result) => {
                self.apply_provider_selection_result(result);
                self.drain_app_events()
            }
            WorkerEvent::GoalMenu { thread_id, result } => {
                if Some(thread_id) == self.active_thread_id {
                    self.apply_goal_menu_result(result);
                }
                self.drain_app_events()
            }
            WorkerEvent::GoalEditor { thread_id, result } => {
                if thread_id.is_none() || thread_id == self.active_thread_id {
                    self.apply_goal_editor_result(thread_id, result);
                }
                self.drain_app_events()
            }
            WorkerEvent::GoalReplaceConfirmation {
                thread_id,
                objective,
            } => {
                if Some(thread_id) == self.active_thread_id {
                    self.show_replace_thread_goal_confirmation(thread_id, objective);
                }
                self.drain_app_events()
            }
            WorkerEvent::GoalObjectiveSet {
                thread_id,
                replacing_goal,
                result,
            } => {
                if Some(thread_id) == self.active_thread_id {
                    self.apply_goal_set_result(
                        result,
                        if replacing_goal { "replace" } else { "set" },
                    );
                }
                self.drain_app_events()
            }
            WorkerEvent::GoalStatusSet { thread_id, result } => {
                if Some(thread_id) == self.active_thread_id {
                    self.apply_goal_set_result(result, "update");
                }
                self.drain_app_events()
            }
            WorkerEvent::GoalCleared { thread_id, result } => {
                if Some(thread_id) == self.active_thread_id {
                    self.apply_goal_clear_result(result);
                }
                self.drain_app_events()
            }
            WorkerEvent::AuditCommandFinished {
                command_text,
                result,
            } => {
                self.apply_audit_command_result(command_text, result);
                self.drain_app_events()
            }
            WorkerEvent::SkillEnabledSet {
                path,
                enabled,
                result,
            } => {
                match result {
                    Ok(()) => {
                        if let Some(chat) = self.chat_widget.as_mut() {
                            chat.update_skill_enabled(path, enabled);
                        }
                    }
                    Err(message) => {
                        if let Some(chat) = self.chat_widget.as_mut() {
                            chat.add_error_message(format!(
                                "Failed to update skill config for {}: {message}",
                                path.display()
                            ));
                        }
                    }
                }
                self.drain_app_events()
            }
            WorkerEvent::SyntaxThemeSelected { name, result } => {
                if let Err(message) = result
                    && let Some(chat) = self.chat_widget.as_mut()
                {
                    chat.add_error_message(format!("Failed to save theme {name}: {message}"));
                }
                self.drain_app_events()
            }
        }
    }

    fn handle_app_server_event(&mut self, event: AppServerEvent) -> ChatAction {
        match event {
            AppServerEvent::Lagged { skipped } => {
                self.status = format!("chat: app-server lagged by {skipped} events");
                self.refresh_mcp_startup_expected_servers();
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_warning_message(self.status.clone());
                    chat.finish_mcp_startup_after_lag();
                }
                self.drain_app_events()
            }
            AppServerEvent::ServerNotification(notification) => {
                self.handle_server_notification(notification);
                self.drain_app_events()
            }
            AppServerEvent::ServerRequest(request) => {
                self.handle_server_request(request);
                self.drain_app_events()
            }
            AppServerEvent::Disconnected { message } => {
                self.status = format!("chat: app-server disconnected: {message}");
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(self.status.clone());
                }
                self.drain_app_events()
            }
        }
    }

    fn handle_server_notification(&mut self, notification: ServerNotification) {
        match &notification {
            ServerNotification::TurnStarted(notification)
                if parsed_thread_id(&notification.thread_id) == self.active_thread_id =>
            {
                self.active_turn_id = Some(notification.turn.id.clone());
                self.status = "chat: turn running".to_string();
            }
            ServerNotification::TurnCompleted(notification)
                if parsed_thread_id(&notification.thread_id) == self.active_thread_id =>
            {
                self.active_turn_id = None;
                self.status = "chat: ready".to_string();
            }
            ServerNotification::ThreadClosed(notification)
                if parsed_thread_id(&notification.thread_id) == self.active_thread_id =>
            {
                self.active_turn_id = None;
                self.status = "chat: thread closed".to_string();
            }
            ServerNotification::TurnStarted(_)
            | ServerNotification::TurnCompleted(_)
            | ServerNotification::ThreadClosed(_) => {}
            ServerNotification::ServerRequestResolved(_) => {}
            ServerNotification::McpServerStatusUpdated(_) => {
                self.refresh_mcp_startup_expected_servers();
            }
            ServerNotification::AccountRateLimitsUpdated(notification) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.on_rate_limit_snapshot(Some(notification.rate_limits.clone()));
                }
                return;
            }
            ServerNotification::AccountUpdated(notification) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.update_account_state(
                        status_account_display_from_auth_mode(
                            notification.auth_mode,
                            notification.plan_type,
                        ),
                        notification.plan_type,
                        matches!(
                            notification.auth_mode,
                            Some(peregrine_app_server_protocol::AuthMode::Chatgpt)
                                | Some(peregrine_app_server_protocol::AuthMode::ChatgptAuthTokens)
                        ),
                    );
                }
                return;
            }
            _ => {}
        }
        if let Some(chat) = self.chat_widget.as_mut() {
            chat.handle_server_notification(notification, None);
        }
    }

    fn handle_server_request(&mut self, request: ServerRequest) {
        self.pending_requests.note_server_request(&request);
        if let Some(chat) = self.chat_widget.as_mut() {
            chat.handle_server_request(request, None);
        }
    }

    fn drain_app_events(&mut self) -> ChatAction {
        let mut action = ChatAction::None;
        for _ in 0..APP_EVENT_DRAIN_LIMIT {
            let event = match self.app_event_rx.as_mut().and_then(|rx| rx.try_recv().ok()) {
                Some(event) => event,
                None => break,
            };
            action = combine_action(action, self.handle_app_event(event));
        }
        action
    }

    fn handle_app_event(&mut self, event: AppEvent) -> ChatAction {
        match event {
            AppEvent::InsertHistoryCell(cell) => {
                let cell: Arc<dyn HistoryCell> = cell.into();
                self.history_cells.push(cell);
                self.transcript_scroll = 0;
                ChatAction::None
            }
            AppEvent::PeregrineOp(op) => self.submit_active_thread_op(op),
            AppEvent::SubmitThreadOp { thread_id, op } => self.submit_thread_op(thread_id, op),
            AppEvent::OpenResumePicker => {
                self.reload_sessions();
                self.mode = HostMode::Sessions;
                ChatAction::None
            }
            AppEvent::ResumeSessionByIdOrName(id_or_name) => {
                self.resume_session_by_id_or_name(id_or_name)
            }
            AppEvent::NewSession | AppEvent::ClearUi => {
                self.history_cells.clear();
                self.start_fresh_session(None, true)
            }
            AppEvent::ClearUiAndSubmitUserMessage { text } => {
                self.history_cells.clear();
                self.start_fresh_session(Some(text), true)
            }
            AppEvent::Exit(_) => ChatAction::Quit,
            AppEvent::SwitchToWorkbench => ChatAction::FocusCode,
            AppEvent::DiffResult(text) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.on_diff_complete();
                    let message = if text.trim().is_empty() {
                        "No changes detected.".to_string()
                    } else {
                        text
                    };
                    chat.add_plain_history_lines(
                        message
                            .lines()
                            .map(|line| Line::raw(line.to_string()))
                            .collect(),
                    );
                }
                self.drain_app_events()
            }
            AppEvent::ThreadHistoryEntryResponse { event, .. } => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.handle_history_entry_response(event);
                }
                ChatAction::None
            }
            AppEvent::LookupMessageHistoryEntry {
                thread_id,
                offset,
                log_id,
            } => {
                if let Some(chat) = self.chat_widget.as_mut()
                    && Some(thread_id) == self.active_thread_id
                {
                    chat.handle_history_entry_response(HistoryLookupResponse {
                        offset,
                        log_id,
                        entry: None,
                    });
                }
                ChatAction::None
            }
            AppEvent::ConsolidateAgentMessage {
                deferred_history_cell,
                ..
            } => {
                if let Some(cell) = deferred_history_cell {
                    let cell: Arc<dyn HistoryCell> = cell.into();
                    self.history_cells.push(cell);
                }
                ChatAction::None
            }
            AppEvent::CommitTick => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.on_commit_tick();
                }
                ChatAction::None
            }
            AppEvent::RefreshRateLimits { origin } => self.refresh_rate_limits(origin),
            AppEvent::RateLimitsLoaded { origin, result } => {
                self.apply_rate_limit_result(origin, result);
                ChatAction::None
            }
            AppEvent::FetchMcpInventory { detail, thread_id } => {
                self.fetch_mcp_inventory(detail, thread_id)
            }
            AppEvent::McpInventoryLoaded {
                result,
                detail,
                thread_id,
            } => {
                self.apply_mcp_inventory_result(result, detail, thread_id);
                ChatAction::None
            }
            AppEvent::OpenProviderPicker => self.open_provider_picker(),
            AppEvent::OpenProviderPopup { providers } => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.open_provider_popup(providers);
                }
                ChatAction::None
            }
            AppEvent::OpenProviderModelPicker {
                provider_id,
                provider_display_name,
            } => self.open_provider_model_picker(provider_id, provider_display_name),
            AppEvent::OpenProviderModelPopup {
                provider_id,
                provider_display_name,
                models,
            } => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.open_provider_model_popup(provider_id, provider_display_name, models);
                }
                ChatAction::None
            }
            AppEvent::PersistProviderSelection { provider_id, model } => {
                self.persist_provider_selection(provider_id, model)
            }
            AppEvent::OpenThreadGoalMenu { thread_id } => self.open_thread_goal_menu(thread_id),
            AppEvent::OpenThreadGoalEditor { thread_id } => self.open_thread_goal_editor(thread_id),
            AppEvent::SetThreadGoalObjective {
                thread_id,
                objective,
                mode,
            } => self.set_thread_goal_objective(thread_id, objective, mode),
            AppEvent::SetThreadGoalStatus { thread_id, status } => {
                self.set_thread_goal_status(thread_id, status)
            }
            AppEvent::ClearThreadGoal { thread_id } => self.clear_thread_goal(thread_id),
            AppEvent::RunAuditCommand {
                command,
                command_text,
            } => self.run_audit_command(command, command_text),
            AppEvent::OpenSkillsList => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.open_skills_list();
                }
                ChatAction::None
            }
            AppEvent::OpenManageSkillsPopup => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.open_manage_skills_popup();
                }
                ChatAction::None
            }
            AppEvent::SetSkillEnabled { path, enabled } => self.set_skill_enabled(path, enabled),
            AppEvent::SyntaxThemeSelected { name } => self.select_syntax_theme(name),
            AppEvent::SyntaxThemePreviewed => ChatAction::None,
            _ => ChatAction::None,
        }
    }

    fn submit_active_thread_op(&mut self, op: AppCommand) -> ChatAction {
        let Some(thread_id) = self.active_thread_id else {
            if matches!(op, AppCommand::UserTurn { .. }) {
                return self.start_fresh_session(None, false);
            }
            if let Some(chat) = self.chat_widget.as_mut() {
                chat.add_error_message("No active chat thread is available.".to_string());
            }
            return self.drain_app_events();
        };
        self.submit_thread_op(thread_id, op)
    }

    fn submit_thread_op(&mut self, thread_id: ThreadId, op: AppCommand) -> ChatAction {
        if let Some(action) = self.try_resolve_pending_request(&op) {
            return action;
        }

        if let AppCommand::ListSkills { cwds, force_reload } = &op {
            return self.submit_list_skills(cwds.clone(), *force_reload);
        }

        if matches!(op, AppCommand::OverrideTurnContext { .. }) {
            return self.sync_override_turn_context_settings(thread_id, &op);
        }

        if !self.send_worker(WorkerCommand::SubmitOp {
            thread_id,
            active_turn_id: self.active_turn_id.clone(),
            op,
        }) && let Some(chat) = self.chat_widget.as_mut()
        {
            chat.add_error_message("chat worker is not ready".to_string());
        }
        ChatAction::None
    }

    fn sync_override_turn_context_settings(
        &mut self,
        thread_id: ThreadId,
        op: &AppCommand,
    ) -> ChatAction {
        let AppCommand::OverrideTurnContext {
            cwd,
            approval_policy,
            approvals_reviewer,
            permission_profile: _,
            active_permission_profile,
            windows_sandbox_level: _,
            model,
            effort,
            summary,
            service_tier,
            collaboration_mode,
            personality,
        } = op
        else {
            return ChatAction::None;
        };

        let params = ThreadSettingsUpdateParams {
            thread_id: thread_id.to_string(),
            cwd: cwd.clone(),
            approval_policy: *approval_policy,
            approvals_reviewer: approvals_reviewer.map(AppServerApprovalsReviewer::from),
            permissions: active_permission_profile
                .as_ref()
                .map(|profile| profile.id.clone()),
            model: model.clone(),
            effort: effort.unwrap_or_default(),
            summary: *summary,
            service_tier: service_tier.clone(),
            collaboration_mode: collaboration_mode.clone(),
            personality: *personality,
            ..ThreadSettingsUpdateParams::default()
        };
        if !thread_settings_update_has_changes(&params) {
            return ChatAction::None;
        }

        self.send_worker(WorkerCommand::SyncSettings(params));
        ChatAction::None
    }

    fn open_thread_goal_menu(&mut self, thread_id: ThreadId) -> ChatAction {
        if Some(thread_id) != self.active_thread_id {
            return ChatAction::None;
        }
        self.send_worker(WorkerCommand::OpenGoalMenu { thread_id });
        ChatAction::None
    }

    fn open_thread_goal_editor(&mut self, thread_id: Option<ThreadId>) -> ChatAction {
        if let Some(thread_id) = thread_id
            && Some(thread_id) != self.active_thread_id
        {
            return ChatAction::None;
        }
        self.send_worker(WorkerCommand::OpenGoalEditor { thread_id });
        ChatAction::None
    }

    fn set_thread_goal_objective(
        &mut self,
        thread_id: ThreadId,
        objective: String,
        mode: ThreadGoalSetMode,
    ) -> ChatAction {
        if Some(thread_id) != self.active_thread_id {
            return ChatAction::None;
        }
        self.send_worker(WorkerCommand::SetGoalObjective {
            thread_id,
            objective,
            mode,
        });
        ChatAction::None
    }

    fn set_thread_goal_status(
        &mut self,
        thread_id: ThreadId,
        status: ThreadGoalStatus,
    ) -> ChatAction {
        if Some(thread_id) != self.active_thread_id {
            return ChatAction::None;
        }
        self.send_worker(WorkerCommand::SetGoalStatus { thread_id, status });
        ChatAction::None
    }

    fn clear_thread_goal(&mut self, thread_id: ThreadId) -> ChatAction {
        if Some(thread_id) != self.active_thread_id {
            return ChatAction::None;
        }
        self.send_worker(WorkerCommand::ClearGoal { thread_id });
        ChatAction::None
    }

    fn run_audit_command(&mut self, command: AuditCommand, command_text: String) -> ChatAction {
        if !self.send_worker(WorkerCommand::RunAuditCommand {
            command,
            command_text,
        }) && let Some(chat) = self.chat_widget.as_mut()
        {
            chat.add_error_message("Audit command failed: app-server unavailable".to_string());
        }
        ChatAction::None
    }

    fn show_replace_thread_goal_confirmation(&mut self, thread_id: ThreadId, objective: String) {
        let replace_objective = objective.clone();
        let replace_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::SetThreadGoalObjective {
                thread_id,
                objective: replace_objective.clone(),
                mode: ThreadGoalSetMode::ReplaceExisting,
            });
        })];
        let items = vec![
            SelectionItem {
                name: "Replace current goal".to_string(),
                description: Some("Set the new objective and start it now".to_string()),
                actions: replace_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Keep the current goal".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        if let Some(chat) = self.chat_widget.as_mut() {
            chat.show_selection_view(SelectionViewParams {
                title: Some("Replace goal?".to_string()),
                subtitle: Some(format!("New objective: {objective}")),
                footer_hint: Some(standard_popup_hint_line()),
                items,
                ..Default::default()
            });
        }
    }

    fn show_no_thread_goal_to_edit(&mut self) {
        if let Some(chat) = self.chat_widget.as_mut() {
            chat.add_error_message("No goal is currently set.".to_string());
            chat.add_info_message(
                "Usage: /goal <objective>".to_string(),
                Some("Create a goal before editing it.".to_string()),
            );
        }
    }

    fn apply_goal_menu_result(
        &mut self,
        result: std::result::Result<ThreadGoalGetResponse, String>,
    ) {
        match result {
            Ok(response) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    if let Some(goal) = response.goal {
                        chat.show_goal_summary(goal);
                    } else {
                        chat.add_info_message(
                            "Usage: /goal <objective>".to_string(),
                            Some("No goal is currently set.".to_string()),
                        );
                    }
                }
            }
            Err(message) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(thread_goal_error_message("read", &message));
                }
            }
        }
    }

    fn apply_goal_editor_result(
        &mut self,
        thread_id: Option<ThreadId>,
        result: std::result::Result<ThreadGoalGetResponse, String>,
    ) {
        let Some(thread_id) = thread_id else {
            self.show_no_thread_goal_to_edit();
            return;
        };
        match result {
            Ok(response) => {
                if let Some(goal) = response.goal {
                    if let Some(chat) = self.chat_widget.as_mut() {
                        chat.show_goal_edit_prompt(thread_id, goal);
                    }
                } else {
                    self.show_no_thread_goal_to_edit();
                }
            }
            Err(message) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(thread_goal_error_message("read", &message));
                }
            }
        }
    }

    fn apply_goal_set_result(
        &mut self,
        result: std::result::Result<ThreadGoalSetResponse, String>,
        action: &str,
    ) {
        match result {
            Ok(response) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_info_message(
                        format!("Goal {}", goal_status_label(response.goal.status)),
                        Some(goal_usage_summary(&response.goal)),
                    );
                }
            }
            Err(message) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(thread_goal_error_message(action, &message));
                }
            }
        }
    }

    fn apply_goal_clear_result(
        &mut self,
        result: std::result::Result<ThreadGoalClearResponse, String>,
    ) {
        match result {
            Ok(response) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    if response.cleared {
                        chat.add_info_message("Goal cleared".to_string(), /*hint*/ None);
                    } else {
                        chat.add_info_message(
                            "No goal to clear".to_string(),
                            Some("This thread does not currently have a goal.".to_string()),
                        );
                    }
                }
            }
            Err(message) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(thread_goal_error_message("clear", &message));
                }
            }
        }
    }

    fn apply_audit_command_result(
        &mut self,
        command_text: String,
        result: std::result::Result<AuditCommandOutput, String>,
    ) {
        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        chat.add_plain_history_lines(vec![command_text.magenta().into()]);
        match result {
            Ok(output) => chat.add_plain_history_lines(output.lines),
            Err(message) => chat.add_error_message(format!("Audit command failed: {message}")),
        }
    }

    fn set_skill_enabled(&mut self, path: AbsolutePathBuf, enabled: bool) -> ChatAction {
        self.send_worker(WorkerCommand::SetSkillEnabled { path, enabled });
        ChatAction::None
    }

    fn refresh_rate_limits(&mut self, origin: RateLimitRefreshOrigin) -> ChatAction {
        self.send_worker(WorkerCommand::RefreshRateLimits { origin });
        ChatAction::None
    }

    fn apply_rate_limit_result(
        &mut self,
        origin: RateLimitRefreshOrigin,
        result: Result<Vec<RateLimitSnapshot>, String>,
    ) {
        match result {
            Ok(snapshots) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    for snapshot in snapshots {
                        chat.on_rate_limit_snapshot(Some(snapshot));
                    }
                    if let RateLimitRefreshOrigin::StatusCommand { request_id } = origin {
                        chat.finish_status_rate_limit_refresh(request_id);
                    }
                }
            }
            Err(err) => {
                self.status = format!("chat: rate limits unavailable: {err}");
                if let Some(chat) = self.chat_widget.as_mut()
                    && let RateLimitRefreshOrigin::StatusCommand { request_id } = origin
                {
                    chat.finish_status_rate_limit_refresh(request_id);
                }
            }
        }
    }

    fn submit_list_skills(&mut self, cwds: Vec<PathBuf>, force_reload: bool) -> ChatAction {
        self.send_worker(WorkerCommand::ListSkills { cwds, force_reload });
        ChatAction::None
    }

    fn try_resolve_pending_request(&mut self, op: &AppCommand) -> Option<ChatAction> {
        match self.pending_requests.take_resolution(op).transpose() {
            Ok(Some((request_id, result))) => {
                self.send_worker(WorkerCommand::ResolveRequest { request_id, result });
            }
            Ok(None) => return None,
            Err(message) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(message);
                }
                return Some(self.drain_app_events());
            }
        }
        Some(ChatAction::None)
    }

    fn resume_session_by_id_or_name(&mut self, id_or_name: String) -> ChatAction {
        if let Ok(thread_id) = ThreadId::from_string(&id_or_name) {
            return self.resume_session(thread_id);
        }
        self.status = format!("chat: looking up {id_or_name}");
        self.send_worker(WorkerCommand::ResumeByIdOrName(id_or_name));
        ChatAction::None
    }

    fn reload_sessions(&mut self) {
        self.status = "chat: loading sessions".to_string();
        self.send_worker(WorkerCommand::ReloadSessions);
    }

    fn select_syntax_theme(&mut self, name: String) -> ChatAction {
        if let Some(context) = self.context.as_mut() {
            context.config.tui_theme = Some(name.clone());
        }
        if let Some(chat) = self.chat_widget.as_mut() {
            chat.set_tui_theme(Some(name.clone()));
        }
        let home = self
            .context
            .as_ref()
            .map(|context| context.config.peregrine_home.as_path());
        if let Some(theme) = crate::agent::resolve_theme_by_name(&name, home) {
            crate::agent::set_syntax_theme(theme);
        }
        if !self.send_worker(WorkerCommand::SelectSyntaxTheme { name: name.clone() })
            && let Some(chat) = self.chat_widget.as_mut()
        {
            chat.add_error_message("chat worker is not ready to save theme".to_string());
        }
        ChatAction::ThemeSelected(name)
    }
}

impl PendingRequests {
    fn note_server_request(&mut self, request: &ServerRequest) {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                self.exec_approvals
                    .insert(params.item_id.clone(), request_id.clone());
                if let Some(approval_id) = &params.approval_id {
                    self.exec_approvals
                        .insert(approval_id.clone(), request_id.clone());
                }
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                self.file_change_approvals
                    .insert(params.item_id.clone(), request_id.clone());
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                self.permission_approvals
                    .insert(params.item_id.clone(), request_id.clone());
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                self.user_inputs
                    .entry(params.turn_id.clone())
                    .or_default()
                    .push_back(PendingUserInputRequest {
                        _item_id: params.item_id.clone(),
                        request_id: request_id.clone(),
                    });
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                self.mcp_requests.push(PendingMcpRequest {
                    server_name: params.server_name.clone(),
                    request_id: request_id.clone(),
                });
            }
            ServerRequest::DynamicToolCall { .. }
            | ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::AttestationGenerate { .. }
            | ServerRequest::ApplyPatchApproval { .. }
            | ServerRequest::ExecCommandApproval { .. } => {}
        }
    }

    fn take_resolution(
        &mut self,
        op: &AppCommand,
    ) -> Option<Result<(AppServerRequestId, serde_json::Value), String>> {
        match op {
            AppCommand::ExecApproval { id, decision, .. } => {
                self.exec_approvals.remove(id).map(|request_id| {
                    serde_json::to_value(CommandExecutionRequestApprovalResponse {
                        decision: decision.clone(),
                    })
                    .map(|result| (request_id, result))
                    .map_err(|err| format!("failed to serialize command approval response: {err}"))
                })
            }
            AppCommand::PatchApproval { id, decision } => {
                self.file_change_approvals.remove(id).map(|request_id| {
                    serde_json::to_value(FileChangeRequestApprovalResponse {
                        decision: decision.clone(),
                    })
                    .map(|result| (request_id, result))
                    .map_err(|err| format!("failed to serialize patch approval response: {err}"))
                })
            }
            AppCommand::UserInputAnswer { id, response } => {
                self.pop_user_input_request_for_turn(id).map(|pending| {
                    serde_json::to_value(response)
                        .map(|result| (pending.request_id, result))
                        .map_err(|err| format!("failed to serialize user input response: {err}"))
                })
            }
            AppCommand::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => self
                .mcp_requests
                .iter()
                .position(|pending| {
                    pending.server_name == *server_name && pending.request_id == *request_id
                })
                .map(|index| {
                    let pending = self.mcp_requests.remove(index);
                    serde_json::to_value(McpServerElicitationRequestResponse {
                        action: *decision,
                        content: content.clone(),
                        meta: meta.clone(),
                    })
                    .map(|result| (pending.request_id, result))
                    .map_err(|err| format!("failed to serialize MCP elicitation response: {err}"))
                }),
            AppCommand::RequestPermissionsResponse { id, response } => {
                self.permission_approvals.remove(id).map(|request_id| {
                    serde_json::to_value(PermissionsRequestApprovalResponse {
                        permissions: granted_permission_profile_from_request(
                            response.permissions.clone(),
                        ),
                        scope: response.scope.into(),
                        strict_auto_review: response.strict_auto_review.then_some(true),
                    })
                    .map(|result| (request_id, result))
                    .map_err(|err| {
                        format!("failed to serialize permissions approval response: {err}")
                    })
                })
            }
            _ => None,
        }
    }

    fn pop_user_input_request_for_turn(
        &mut self,
        turn_id: &str,
    ) -> Option<PendingUserInputRequest> {
        let queue = self.user_inputs.get_mut(turn_id)?;
        let pending = queue.pop_front();
        if queue.is_empty() {
            self.user_inputs.remove(turn_id);
        }
        pending
    }
}

struct Startup {
    app_server: AppServerSession,
    context: ChatContext,
    sessions: Vec<SessionRow>,
}

async fn start_worker(
    root: PathBuf,
    mut initial_config: Option<Config>,
    mut app_server: Option<AppServerSession>,
    preserve_ui_state: bool,
    app_event_tx: AppEventSender,
    event_tx: UnboundedSender<WorkerEvent>,
) {
    if preserve_ui_state
        && let (Some(app_server), Some(config)) = (app_server.take(), initial_config.take())
    {
        let (command_tx, command_rx) = unbounded_channel();
        let _ = event_tx.send(WorkerEvent::ConnectionResumed { command_tx });
        run_worker(app_server, config, command_rx, event_tx).await;
        return;
    }

    let startup = match start_host(root, initial_config, app_server, app_event_tx).await {
        Ok(startup) => startup,
        Err(err) => {
            let _ = event_tx.send(WorkerEvent::StartupFailed(format!("{err:#}")));
            return;
        }
    };
    let (command_tx, command_rx) = unbounded_channel();
    let context = startup.context.clone();
    let config = startup.context.config.clone();
    let _ = event_tx.send(WorkerEvent::Started {
        command_tx,
        context,
        sessions: startup.sessions,
    });
    run_worker(startup.app_server, config, command_rx, event_tx).await;
}

async fn run_worker(
    mut app_server: AppServerSession,
    mut config: Config,
    mut command_rx: UnboundedReceiver<WorkerCommand>,
    event_tx: UnboundedSender<WorkerEvent>,
) {
    loop {
        tokio::select! {
            event = app_server.next_event() => {
                match event {
                    Some(event) => {
                        if event_tx.send(WorkerEvent::AppServer(event)).is_err() {
                            let _ = app_server.shutdown().await;
                            return;
                        }
                    }
                    None => {
                        let _ = event_tx.send(WorkerEvent::Stopped("app-server disconnected".to_string()));
                        return;
                    }
                }
            }
            command = command_rx.recv() => {
                match command {
                    Some(WorkerCommand::Shutdown) | None => {
                        let _ = app_server.shutdown().await;
                        return;
                    }
                    Some(WorkerCommand::Suspend { response_tx }) => {
                        let _ = response_tx.send(app_server);
                        return;
                    }
                    Some(command) => {
                        handle_worker_command(command, &mut app_server, &mut config, &event_tx).await;
                    }
                }
            }
        }
    }
}

async fn handle_worker_command(
    command: WorkerCommand,
    app_server: &mut AppServerSession,
    config: &mut Config,
    event_tx: &UnboundedSender<WorkerEvent>,
) {
    match command {
        WorkerCommand::Shutdown => {}
        WorkerCommand::Suspend { .. } => {
            unreachable!("suspend commands are handled by the worker event loop")
        }
        WorkerCommand::StartFresh {
            initial_text,
            replace_widget,
        } => {
            let result = app_server
                .start_thread_with_session_start_source(config, None)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::FreshStarted {
                initial_text,
                replace_widget,
                result,
            });
        }
        WorkerCommand::Resume { thread_id } => {
            let result = app_server
                .resume_thread(config.clone(), thread_id)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::Resumed { result });
        }
        WorkerCommand::ReloadSessions => {
            let result = load_sessions(app_server, config)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::SessionsLoaded(result));
        }
        WorkerCommand::ResumeByIdOrName(id_or_name) => {
            let result = lookup_session_by_exact_name(app_server, config, &id_or_name)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::SessionLookup { id_or_name, result });
        }
        WorkerCommand::SubmitOp {
            thread_id,
            active_turn_id,
            op,
        } => {
            let result = submit_op_to_app_server(app_server, config, active_turn_id, thread_id, op)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::OpSubmitted(result));
        }
        WorkerCommand::ResolveRequest { request_id, result } => {
            let result = app_server
                .resolve_server_request(request_id, result)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::RequestResolved(result));
        }
        WorkerCommand::ListSkills { cwds, force_reload } => {
            let result = app_server
                .skills_list(SkillsListParams { cwds, force_reload })
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::SkillsListed(result));
        }
        WorkerCommand::SyncSettings(params) => {
            let result = app_server
                .thread_settings_update(params)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::SettingsSynced(result));
        }
        WorkerCommand::RefreshRateLimits { origin } => {
            let request_handle = app_server.request_handle();
            let result = async {
                let request_id =
                    RequestId::String(format!("workbench-chat-rate-limits-{}", Uuid::new_v4()));
                let response: GetAccountRateLimitsResponse = request_handle
                    .request_typed(ClientRequest::GetAccountRateLimits {
                        request_id,
                        params: None,
                    })
                    .await
                    .wrap_err("account/rateLimits/read failed in workbench chat")?;
                Ok::<_, color_eyre::Report>(app_server_rate_limit_snapshots(response))
            }
            .await
            .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::RateLimitsLoaded { origin, result });
        }
        WorkerCommand::FetchMcpInventory { detail, thread_id } => {
            let request_handle = app_server.request_handle();
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let result = crate::session::fetch_all_mcp_server_statuses(
                    request_handle,
                    detail,
                    thread_id,
                )
                .await
                .map_err(report_string);
                let _ = event_tx.send(WorkerEvent::McpInventoryLoaded {
                    detail,
                    thread_id,
                    result,
                });
            });
        }
        WorkerCommand::LoadProviders => {
            let request_handle = app_server.request_handle();
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let request_id =
                    RequestId::String(format!("workbench-provider-list-{}", Uuid::new_v4()));
                let result = request_handle
                    .request_typed::<ModelProviderListResponse>(ClientRequest::ModelProviderList {
                        request_id,
                        params: ModelProviderListParams {},
                    })
                    .await
                    .map_err(report_string);
                let _ = event_tx.send(WorkerEvent::ProvidersLoaded(result));
            });
        }
        WorkerCommand::LoadProviderModels {
            provider_id,
            provider_display_name,
        } => {
            let request_handle = app_server.request_handle();
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let request_id =
                    RequestId::String(format!("workbench-provider-models-{}", Uuid::new_v4()));
                let result = request_handle
                    .request_typed::<ModelProviderModelsListResponse>(
                        ClientRequest::ModelProviderModelsList {
                            request_id,
                            params: ModelProviderModelsListParams {
                                provider_id: provider_id.clone(),
                            },
                        },
                    )
                    .await
                    .map_err(report_string);
                let _ = event_tx.send(WorkerEvent::ProviderModelsLoaded {
                    provider_id,
                    provider_display_name,
                    result,
                });
            });
        }
        WorkerCommand::SelectProvider { provider_id, model } => {
            let request_id =
                RequestId::String(format!("workbench-provider-select-{}", Uuid::new_v4()));
            let result = app_server
                .request_handle()
                .request_typed::<ModelProviderSelectResponse>(ClientRequest::ModelProviderSelect {
                    request_id,
                    params: ModelProviderSelectParams {
                        provider_id: provider_id.clone(),
                        model,
                    },
                })
                .await
                .map_err(report_string);
            if let Ok(response) = result.as_ref() {
                provider::apply_provider_selection_to_config(
                    config,
                    &response.selected_provider,
                    response.model.as_deref(),
                );
            }
            let _ = event_tx.send(WorkerEvent::ProviderSelected(result));
        }
        WorkerCommand::OpenGoalMenu { thread_id } => {
            let result = app_server
                .thread_goal_get(thread_id)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::GoalMenu { thread_id, result });
        }
        WorkerCommand::OpenGoalEditor { thread_id } => {
            let result = match thread_id {
                Some(thread_id) => app_server
                    .thread_goal_get(thread_id)
                    .await
                    .map_err(report_string),
                None => Ok(ThreadGoalGetResponse { goal: None }),
            };
            let _ = event_tx.send(WorkerEvent::GoalEditor { thread_id, result });
        }
        WorkerCommand::SetGoalObjective {
            thread_id,
            objective,
            mode,
        } => {
            set_goal_objective(app_server, event_tx, thread_id, objective, mode).await;
        }
        WorkerCommand::SetGoalStatus { thread_id, status } => {
            let result = app_server
                .thread_goal_set(thread_id, None, Some(status), None)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::GoalStatusSet { thread_id, result });
        }
        WorkerCommand::ClearGoal { thread_id } => {
            let result = app_server
                .thread_goal_clear(thread_id)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::GoalCleared { thread_id, result });
        }
        WorkerCommand::RunAuditCommand {
            command,
            command_text,
        } => {
            let result = execute_audit_command(app_server, command)
                .await
                .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::AuditCommandFinished {
                command_text,
                result,
            });
        }
        WorkerCommand::SetSkillEnabled { path, enabled } => {
            let request_handle = app_server.request_handle();
            let result = crate::agent::config_update::write_skill_enabled(
                request_handle,
                path.clone(),
                enabled,
            )
            .await
            .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::SkillEnabledSet {
                path,
                enabled,
                result,
            });
        }
        WorkerCommand::SelectSyntaxTheme { name } => {
            if let Some(theme) =
                crate::agent::resolve_theme_by_name(&name, Some(&config.peregrine_home))
            {
                crate::agent::set_syntax_theme(theme);
            }
            let edit = crate::agent::legacy_core::config::edit::syntax_theme_edit(&name);
            let result =
                crate::agent::legacy_core::config::edit::ConfigEditsBuilder::for_config(config)
                    .with_edits([edit])
                    .apply()
                    .await
                    .map(|()| {
                        config.tui_theme = Some(name.clone());
                    })
                    .map_err(report_string);
            let _ = event_tx.send(WorkerEvent::SyntaxThemeSelected { name, result });
        }
    }
}

async fn set_goal_objective(
    app_server: &mut AppServerSession,
    event_tx: &UnboundedSender<WorkerEvent>,
    thread_id: ThreadId,
    objective: String,
    mode: ThreadGoalSetMode,
) {
    let mut mode = mode;
    if matches!(mode, ThreadGoalSetMode::ConfirmIfExists) {
        match app_server.thread_goal_get(thread_id).await {
            Ok(response) => match response.goal.as_ref() {
                Some(goal) if should_confirm_before_replacing_goal(goal) => {
                    let _ = event_tx.send(WorkerEvent::GoalReplaceConfirmation {
                        thread_id,
                        objective,
                    });
                    return;
                }
                Some(_) => {
                    mode = ThreadGoalSetMode::ReplaceExisting;
                }
                None => {}
            },
            Err(err) => {
                let _ = event_tx.send(WorkerEvent::GoalObjectiveSet {
                    thread_id,
                    replacing_goal: false,
                    result: Err(report_string(err)),
                });
                return;
            }
        }
    }

    let replacing_goal = matches!(mode, ThreadGoalSetMode::ReplaceExisting);
    if replacing_goal && let Err(err) = app_server.thread_goal_clear(thread_id).await {
        let _ = event_tx.send(WorkerEvent::GoalObjectiveSet {
            thread_id,
            replacing_goal,
            result: Err(report_string(err)),
        });
        return;
    }

    let (status, token_budget) = match mode {
        ThreadGoalSetMode::ConfirmIfExists | ThreadGoalSetMode::ReplaceExisting => {
            (ThreadGoalStatus::Active, None)
        }
        ThreadGoalSetMode::UpdateExisting {
            status,
            token_budget,
        } => (status, Some(token_budget)),
    };
    let result = app_server
        .thread_goal_set(thread_id, Some(objective), Some(status), token_budget)
        .await
        .map_err(report_string);
    let _ = event_tx.send(WorkerEvent::GoalObjectiveSet {
        thread_id,
        replacing_goal,
        result,
    });
}

async fn lookup_session_by_exact_name(
    app_server: &mut AppServerSession,
    config: &Config,
    id_or_name: &str,
) -> Result<Option<SessionRow>> {
    let response = app_server
        .thread_list(thread_list_params(
            config,
            None,
            Some(id_or_name.to_string()),
        ))
        .await?;
    Ok(response
        .data
        .into_iter()
        .find(|thread| thread.name.as_deref() == Some(id_or_name))
        .and_then(session_row_from_thread))
}

async fn start_host(
    root: PathBuf,
    initial_config: Option<Config>,
    existing_app_server: Option<AppServerSession>,
    app_event_tx: AppEventSender,
) -> Result<Startup> {
    let config = match initial_config {
        Some(config) => config,
        None => ConfigBuilder::default()
            .harness_overrides(ConfigOverrides {
                cwd: Some(root),
                peregrine_self_exe: std::env::current_exe().ok(),
                ..ConfigOverrides::default()
            })
            .loader_overrides(LoaderOverrides::default())
            .strict_config(false)
            .cloud_requirements(CloudRequirementsLoader::default())
            .build()
            .await
            .wrap_err("failed to load agent config for workbench chat")?,
    };

    let mut app_server = match existing_app_server {
        Some(app_server) => app_server,
        None => {
            let arg0_paths = crate::agent_arg0_dispatch_paths()
                .wrap_err("failed to resolve Peregrine executable paths")?;
            let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
                arg0_paths.codex_self_exe.clone(),
                arg0_paths.codex_linux_sandbox_exe.clone(),
            )
            .wrap_err("failed to build exec-server runtime paths")?;
            let environment_manager = EnvironmentManager::from_env(Some(local_runtime_paths))
                .await
                .map(Arc::new)
                .map_err(color_eyre::Report::from)
                .wrap_err("failed to load workbench chat environment manager")?;
            crate::agent::start_app_server_for_picker(
                &config,
                &AppServerTarget::Embedded,
                arg0_paths,
                None,
                environment_manager,
            )
            .await
            .wrap_err("failed to start embedded app server for workbench chat")?
        }
    };
    let bootstrap = app_server
        .bootstrap(&config)
        .await
        .wrap_err("failed to bootstrap workbench chat app server")?;
    let sessions = load_sessions(&mut app_server, &config)
        .await
        .unwrap_or_default();
    let context = chat_context(config, bootstrap, app_event_tx);

    Ok(Startup {
        app_server,
        context,
        sessions,
    })
}

fn chat_context(
    config: Config,
    bootstrap: AppServerBootstrap,
    app_event_tx: AppEventSender,
) -> ChatContext {
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| bootstrap.default_model.clone());
    ChatContext {
        config,
        model_catalog: Arc::new(ModelCatalog::new(bootstrap.available_models)),
        app_event_tx,
        has_chatgpt_account: bootstrap.has_chatgpt_account,
        status_account_display: bootstrap.status_account_display,
        initial_plan_type: bootstrap.plan_type,
        model,
        account_email: bootstrap.account_email,
        auth_mode: bootstrap.auth_mode,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
    }
}

fn enabled_mcp_server_names(config: &Config) -> Vec<String> {
    config
        .mcp_servers
        .get()
        .iter()
        .filter_map(|(name, server)| server.enabled.then_some(name.clone()))
        .collect()
}

fn new_session_telemetry(context: &ChatContext) -> SessionTelemetry {
    SessionTelemetry::new(
        ThreadId::new(),
        context.model.as_str(),
        context.model.as_str(),
        None,
        context.account_email.clone(),
        context.auth_mode,
        "peregrine-tui".to_string(),
        context.config.otel.log_user_prompt,
        user_agent(),
        serde_json::from_value(serde_json::json!("cli"))
            .unwrap_or_else(|err| panic!("cli session source should deserialize: {err}")),
    )
}

async fn load_sessions(
    app_server: &mut AppServerSession,
    config: &Config,
) -> Result<Vec<SessionRow>> {
    let cwd_response = app_server
        .thread_list(thread_list_params(config, Some(config.cwd.as_path()), None))
        .await?;
    let mut rows = cwd_response
        .data
        .into_iter()
        .filter_map(session_row_from_thread)
        .collect::<Vec<_>>();
    if rows.is_empty() {
        let all_response = app_server
            .thread_list(thread_list_params(config, None, None))
            .await?;
        rows = all_response
            .data
            .into_iter()
            .filter_map(session_row_from_thread)
            .collect();
    }
    Ok(rows)
}

fn thread_list_params(
    config: &Config,
    cwd: Option<&Path>,
    search_term: Option<String>,
) -> ThreadListParams {
    ThreadListParams {
        cursor: None,
        limit: Some(SESSION_PAGE_SIZE),
        sort_key: Some(ThreadSortKey::UpdatedAt),
        sort_direction: None,
        model_providers: Some(vec![config.model_provider_id.clone()]),
        source_kinds: Some(resume_source_kinds(false)),
        archived: Some(false),
        cwd: cwd.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().to_string())),
        use_state_db_only: false,
        search_term,
    }
}

fn session_row_from_thread(thread: Thread) -> Option<SessionRow> {
    let thread_id = ThreadId::from_string(&thread.id).ok()?;
    let title = thread
        .name
        .or_else(|| {
            let preview = thread.preview.trim();
            (!preview.is_empty()).then(|| preview.to_string())
        })
        .unwrap_or_else(|| format!("thread {thread_id}"));
    Some(SessionRow {
        thread_id,
        title,
        cwd: thread.cwd.to_path_buf(),
        updated_at: thread.updated_at,
    })
}

async fn submit_op_to_app_server(
    app_server: &mut AppServerSession,
    config: &Config,
    active_turn_id: Option<String>,
    thread_id: ThreadId,
    op: AppCommand,
) -> Result<()> {
    match op {
        AppCommand::Interrupt => {
            if let Some(turn_id) = active_turn_id {
                app_server.turn_interrupt(thread_id, turn_id).await?;
            } else {
                app_server.startup_interrupt(thread_id).await?;
            }
        }
        AppCommand::UserTurn {
            items,
            cwd,
            approval_policy,
            approvals_reviewer,
            active_permission_profile,
            model,
            effort,
            summary,
            service_tier,
            final_output_json_schema,
            collaboration_mode,
            personality,
        } => {
            if let Some(turn_id) = active_turn_id {
                app_server.turn_steer(thread_id, turn_id, items).await?;
            } else {
                app_server
                    .turn_start(
                        thread_id,
                        items,
                        cwd,
                        approval_policy,
                        approvals_reviewer.unwrap_or(config.approvals_reviewer),
                        active_permission_profile
                            .map(TurnPermissionsOverride::ActiveProfile)
                            .unwrap_or(TurnPermissionsOverride::Preserve),
                        config.permissions.user_visible_workspace_roots(),
                        model,
                        effort,
                        summary,
                        service_tier,
                        collaboration_mode,
                        personality,
                        final_output_json_schema,
                    )
                    .await?;
            }
        }
        AppCommand::ListSkills { cwds, force_reload } => {
            let response = app_server
                .skills_list(SkillsListParams { cwds, force_reload })
                .await?;
            let _ = response;
        }
        AppCommand::Compact => {
            app_server.thread_compact_start(thread_id).await?;
        }
        AppCommand::SetThreadName { name } => {
            app_server.thread_set_name(thread_id, name).await?;
        }
        AppCommand::RunUserShellCommand { command } => {
            app_server.thread_shell_command(thread_id, command).await?;
        }
        AppCommand::Review { target } => {
            app_server.review_start(thread_id, target).await?;
        }
        AppCommand::CleanBackgroundTerminals => {
            app_server
                .thread_background_terminals_clean(thread_id)
                .await?;
        }
        AppCommand::ReloadUserConfig => {
            app_server.reload_user_config().await?;
        }
        AppCommand::ApproveGuardianDeniedAction { event } => {
            app_server
                .thread_approve_guardian_denied_action(thread_id, &event)
                .await?;
        }
        AppCommand::RealtimeConversationStart { transport, voice } => {
            app_server
                .thread_realtime_start(thread_id, transport, voice)
                .await?;
        }
        AppCommand::RealtimeConversationAudio(frame) => {
            app_server.thread_realtime_audio(thread_id, frame).await?;
        }
        AppCommand::RealtimeConversationClose => {
            app_server.thread_realtime_stop(thread_id).await?;
        }
        AppCommand::ThreadRollback { .. }
        | AppCommand::OverrideTurnContext { .. }
        | AppCommand::ExecApproval { .. }
        | AppCommand::PatchApproval { .. }
        | AppCommand::ResolveElicitation { .. }
        | AppCommand::UserInputAnswer { .. }
        | AppCommand::RequestPermissionsResponse { .. }
        | AppCommand::Shutdown => {}
    }
    Ok(())
}

fn session_list_key_action(key: KeyEvent) -> SessionListKeyAction {
    let plain = key.modifiers == KeyModifiers::NONE;
    match key.code {
        KeyCode::Enter if plain => SessionListKeyAction::ResumeSelected,
        KeyCode::Esc if plain => SessionListKeyAction::StartFresh,
        KeyCode::Up | KeyCode::Char('k') if plain => SessionListKeyAction::Previous,
        KeyCode::Down | KeyCode::Char('j') if plain => SessionListKeyAction::Next,
        _ => SessionListKeyAction::PassToComposer,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionListKeyAction {
    Previous,
    Next,
    ResumeSelected,
    StartFresh,
    PassToComposer,
}

impl Drop for ChatController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn shutdown_owned_runtime(runtime: Arc<tokio::runtime::Runtime>) {
    if let Ok(runtime) = Arc::try_unwrap(runtime) {
        runtime.shutdown_timeout(Duration::from_millis(250));
    }
}

fn combine_action(current: ChatAction, next: ChatAction) -> ChatAction {
    match (current, next) {
        (ChatAction::Quit, _) | (_, ChatAction::Quit) => ChatAction::Quit,
        (ChatAction::FocusCode, _) | (_, ChatAction::FocusCode) => ChatAction::FocusCode,
        (ChatAction::ThemeSelected(_), ChatAction::ThemeSelected(name))
        | (ChatAction::None, ChatAction::ThemeSelected(name))
        | (ChatAction::ThemeSelected(name), ChatAction::None) => ChatAction::ThemeSelected(name),
        _ => ChatAction::None,
    }
}

fn chat_base_style(palette: ThemePalette) -> Style {
    Style::default().fg(palette.fg).bg(palette.bg)
}

fn chat_muted_style(palette: ThemePalette) -> Style {
    Style::default().fg(palette.muted).bg(palette.bg)
}

fn chat_selection_style(palette: ThemePalette) -> Style {
    Style::default()
        .fg(palette.fg)
        .bg(palette.selection)
        .add_modifier(Modifier::BOLD)
}

fn chat_block(title: impl Into<String>, focused: bool, palette: ThemePalette) -> Block<'static> {
    let border = if focused {
        palette.accent
    } else {
        palette.graph.edge
    };
    let title_style = Style::default()
        .fg(if focused { palette.accent } else { palette.fg })
        .bg(palette.bg)
        .add_modifier(if focused {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });
    Block::default()
        .borders(Borders::ALL)
        .title(title.into())
        .style(chat_base_style(palette))
        .border_style(Style::default().fg(border).bg(palette.bg))
        .title_style(title_style)
}

fn format_updated_at(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|utc| {
            utc.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn parsed_thread_id(thread_id: &str) -> Option<ThreadId> {
    ThreadId::from_string(thread_id).ok()
}

fn report_string<E: std::fmt::Display>(err: E) -> String {
    format!("{err:#}")
}

fn thread_goal_error_message(action: &str, err: &str) -> String {
    if is_ephemeral_thread_goal_error(err) {
        EPHEMERAL_THREAD_GOAL_ERROR_MESSAGE.to_string()
    } else {
        format!("Failed to {action} thread goal: {err}")
    }
}

fn is_ephemeral_thread_goal_error(err: &str) -> bool {
    err.contains("ephemeral thread does not support goals")
        || err.contains("thread goals require a persisted thread; this thread is ephemeral")
}

fn should_confirm_before_replacing_goal(goal: &ThreadGoal) -> bool {
    match goal.status {
        ThreadGoalStatus::Complete => false,
        ThreadGoalStatus::Active
        | ThreadGoalStatus::Paused
        | ThreadGoalStatus::Blocked
        | ThreadGoalStatus::UsageLimited
        | ThreadGoalStatus::BudgetLimited => true,
    }
}

fn thread_settings_update_has_changes(params: &ThreadSettingsUpdateParams) -> bool {
    params.cwd.is_some()
        || params.approval_policy.is_some()
        || params.approvals_reviewer.is_some()
        || params.sandbox_policy.is_some()
        || params.permissions.is_some()
        || params.model.is_some()
        || params.service_tier.is_some()
        || params.effort.is_some()
        || params.summary.is_some()
        || params.collaboration_mode.is_some()
        || params.personality.is_some()
}

fn u16_saturating(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn status_account_display_from_auth_mode(
    auth_mode: Option<peregrine_app_server_protocol::AuthMode>,
    plan_type: Option<peregrine_types::account::PlanType>,
) -> Option<StatusAccountDisplay> {
    match auth_mode {
        Some(peregrine_app_server_protocol::AuthMode::ApiKey) => Some(StatusAccountDisplay::ApiKey),
        Some(peregrine_app_server_protocol::AuthMode::Chatgpt)
        | Some(peregrine_app_server_protocol::AuthMode::ChatgptAuthTokens) => {
            Some(StatusAccountDisplay::ChatGpt {
                email: None,
                plan: plan_type.map(|plan| format!("{plan:?}")),
            })
        }
        Some(peregrine_app_server_protocol::AuthMode::AgentIdentity) | None => None,
    }
}
