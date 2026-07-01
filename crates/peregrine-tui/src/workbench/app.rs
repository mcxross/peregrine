use super::{
    AppMode, BytecodeCacheEntry, BytecodeLoadResult, BytecodePane, BytecodeTargetKey,
    CloseConfirmation, EditorMode, EditorRenderCache, EditorWorkspace, Explorer, FocusPane,
    GraphLoadResult, GraphPanes, GraphTab, StartupTaskResult, VimState, WorkbenchExit,
    WorkbenchLayout, WorkbenchStartupState, WorkbenchTab,
};
use crate::agent;
use crate::app;
use crate::chat;
use crate::navigation::Navigation;
use crate::sui::package_loader::PackageLoadReport;
use crate::sui::project::CliContext;
use crate::theme::ThemeState;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;

pub struct App {
    pub(crate) application_runtime: Option<app::ApplicationRuntime>,
    pub(crate) application_config: Option<std::sync::Arc<agent::legacy_core::config::Config>>,
    pub(crate) mode: AppMode,
    pub(crate) focus: FocusPane,
    pub(crate) active_tab: WorkbenchTab,
    pub(crate) editor_mode: EditorMode,
    pub(crate) standard_editor_editing: bool,
    pub(crate) vim_state: VimState,
    pub(crate) theme: ThemeState,
    pub(crate) theme_generation: u64,
    pub(crate) navigation: Navigation,
    pub(crate) explorer: Explorer,
    pub(crate) editor: EditorWorkspace,
    pub(crate) editor_render_cache: Option<EditorRenderCache>,
    pub(crate) pending_close: Option<CloseConfirmation>,
    pub(crate) bytecode: BytecodePane,
    pub(crate) bytecode_cache: HashMap<BytecodeTargetKey, BytecodeCacheEntry>,
    pub(crate) bytecode_loader_rx: Option<mpsc::Receiver<BytecodeLoadResult>>,
    pub(crate) bytecode_load_epoch: u64,
    pub(crate) graphs: GraphPanes,
    pub(crate) graph_loader_rx: Option<(GraphTab, mpsc::Receiver<GraphLoadResult>)>,
    pub(crate) chat: chat::ChatController,
    pub(crate) startup: WorkbenchStartupState,
    pub(crate) startup_task_rx: Option<mpsc::Receiver<StartupTaskResult>>,
    pub(crate) package_load_report: Option<PackageLoadReport>,
    pub(crate) created_package_trust_persister: fn(&Path) -> Result<(), String>,
    pub(crate) package_loader: fn(CliContext) -> PackageLoadReport,
    pub(crate) exit: Option<WorkbenchExit>,
    pub(crate) status: String,
    pub(crate) layout: WorkbenchLayout,
}
