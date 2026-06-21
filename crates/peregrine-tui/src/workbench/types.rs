use crate::sui::package_loader::{
    PackageCreateReport, PackageLoadReport, WorkbenchTrustResolution,
};
use crate::sui::project::CliContext;
use crate::workbench::command_input::CommandInput;
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Workbench,
    Agent,
}

impl Default for AppMode {
    fn default() -> Self {
        Self::Workbench
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Explorer,
    Tabs,
    FileTabs,
    Editor,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchTab {
    Chat,
    Editor,
    Bytecode,
    Graphs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphTab {
    Cfg,
    CallGraph,
    TypeGraph,
}

impl GraphTab {
    pub(crate) const ALL: [Self; 3] = [Self::Cfg, Self::CallGraph, Self::TypeGraph];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Cfg => "cfg",
            Self::CallGraph => "call graph",
            Self::TypeGraph => "type graph",
        }
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }
}

impl WorkbenchTab {
    pub(crate) const ALL: [Self; 4] = [
        Self::Chat,
        Self::Editor,
        Self::Bytecode,
        Self::Graphs,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Editor => "editor",
            Self::Bytecode => "bytecode",
            Self::Graphs => "graphs",
        }
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }
}

impl Default for WorkbenchTab {
    fn default() -> Self {
        Self::Chat
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Standard,
    Vim,
}

impl Default for EditorMode {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimState {
    Normal,
    Insert,
}

impl Default for VimState {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchExit {
    Quit,
    SwitchToAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvalidPackageAction {
    CreatePackage,
    ProceedAnyway,
    GoBack,
}

impl InvalidPackageAction {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::CreatePackage => Self::ProceedAnyway,
            Self::ProceedAnyway => Self::GoBack,
            Self::GoBack => Self::CreatePackage,
        }
    }
    
    pub(crate) fn toggle_back(self) -> Self {
        match self {
            Self::CreatePackage => Self::GoBack,
            Self::ProceedAnyway => Self::CreatePackage,
            Self::GoBack => Self::ProceedAnyway,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InvalidPackagePrompt {
    pub(crate) root: PathBuf,
    pub(crate) message: String,
    pub(crate) trust_resolution: WorkbenchTrustResolution,
    pub(crate) selected: InvalidPackageAction,
}

#[derive(Debug, Clone)]
pub(crate) struct PackageNamePrompt {
    pub(crate) parent: PathBuf,
    pub(crate) input: CommandInput,
    pub(crate) error: Option<String>,
    pub(crate) trust_resolution: WorkbenchTrustResolution,
    pub(crate) invalid_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrustAction {
    Trust,
    ContinueWithoutTrust,
}

impl TrustAction {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Trust => Self::ContinueWithoutTrust,
            Self::ContinueWithoutTrust => Self::Trust,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TrustPrompt {
    pub(crate) resolution: WorkbenchTrustResolution,
    pub(crate) post_action: TrustPostAction,
    pub(crate) selected: TrustAction,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum TrustPostAction {
    EnterWorkbench,
    LoadPackage(CliContext),
}

#[derive(Debug, Clone)]
pub(crate) struct PackageLoadRunningState {
    pub(crate) message: String,
    pub(crate) started_at: Instant,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkbenchStartupState {
    Workbench,
    InvalidPackageChoice(InvalidPackagePrompt),
    PackageNameEntry(PackageNamePrompt),
    TrustDecision(TrustPrompt),
    PackageLoadRunning(PackageLoadRunningState),
}

impl WorkbenchStartupState {
    pub(crate) fn is_workbench(&self) -> bool {
        matches!(self, Self::Workbench | Self::PackageLoadRunning(_))
    }
}

pub(crate) enum StartupTaskResult {
    CreatePackage {
        parent: PathBuf,
        package_name: String,
        trust_resolution: WorkbenchTrustResolution,
        invalid_message: String,
        report: PackageCreateReport,
    },
    LoadPackage(PackageLoadReport),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileTabHitTarget {
    Previous,
    Activate(crate::workbench::DocumentId),
    Close(crate::workbench::DocumentId),
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseChoice {
    Save,
    Discard,
    Cancel,
}

impl CloseChoice {
    pub(crate) const ALL: [Self; 3] = [Self::Save, Self::Discard, Self::Cancel];

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Save => Self::Discard,
            Self::Discard => Self::Cancel,
            Self::Cancel => Self::Save,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Save => Self::Cancel,
            Self::Discard => Self::Save,
            Self::Cancel => Self::Discard,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Save => "Save",
            Self::Discard => "Discard",
            Self::Cancel => "Cancel",
        }
    }

    pub(crate) fn shortcut(self) -> &'static str {
        match self {
            Self::Save => "S",
            Self::Discard => "D",
            Self::Cancel => "C",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloseConfirmation {
    pub(crate) document_id: crate::workbench::DocumentId,
    pub(crate) selected: CloseChoice,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileTabHitArea {
    pub(crate) target: FileTabHitTarget,
    pub(crate) area: Rect,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct WorkbenchLayout {
    pub(crate) explorer: Rect,
    pub(crate) tabs: Rect,
    pub(crate) tab_hit_areas: Vec<(WorkbenchTab, Rect)>,
    pub(crate) file_tabs: Rect,
    pub(crate) file_tab_hit_areas: Vec<FileTabHitArea>,
    pub(crate) graph_tabs: Rect,
    pub(crate) graph_tab_hit_areas: Vec<(GraphTab, Rect)>,
    pub(crate) close_dialog_hit_areas: Vec<(CloseChoice, Rect)>,
    pub(crate) editor: Rect,
    pub(crate) bottom_bar: Rect,
}
