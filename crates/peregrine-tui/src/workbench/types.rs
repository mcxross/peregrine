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
    Editor,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchTab {
    Code,
    Bytecode,
    Cfg,
    CallGraph,
    TypeGraph,
    Chat,
}

impl WorkbenchTab {
    pub(crate) const ALL: [Self; 6] = [
        Self::Code,
        Self::Bytecode,
        Self::Cfg,
        Self::CallGraph,
        Self::TypeGraph,
        Self::Chat,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Bytecode => "bytecode",
            Self::Cfg => "cfg",
            Self::CallGraph => "call graph",
            Self::TypeGraph => "type graph",
            Self::Chat => "chat",
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
        Self::Code
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
}

impl InvalidPackageAction {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::CreatePackage => Self::ProceedAnyway,
            Self::ProceedAnyway => Self::CreatePackage,
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

#[derive(Debug, Default, Clone)]
pub(crate) struct WorkbenchLayout {
    pub(crate) explorer: Rect,
    pub(crate) tabs: Rect,
    pub(crate) tab_hit_areas: Vec<(WorkbenchTab, Rect)>,
    pub(crate) editor: Rect,
    pub(crate) input: Rect,
    pub(crate) bottom_bar: Rect,
}
