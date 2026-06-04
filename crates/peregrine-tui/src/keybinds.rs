use std::io;

/// Key binding events for the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBindEvent {
    Quit,
    BeginWorkbenchNavigation,
    Save,
    Reload,
    Undo,
    FocusNext,
    FocusPrevious,
    SelectCodeTab,
    SelectBytecodeTab,
    SelectCfgTab,
    SelectCallGraphTab,
    SelectTypeGraphTab,
    WorkbenchCancel,
    WorkbenchFocusLeft,
    WorkbenchFocusDown,
    WorkbenchFocusUp,
    WorkbenchFocusRight,
    WorkbenchFocusExplorer,
    WorkbenchFocusTabs,
    WorkbenchFocusCodeEditor,
    WorkbenchFocusInput,
    WorkbenchFocusInspector,
    WorkbenchToggleEditorMode,
    WorkbenchPreviousTheme,
    WorkbenchNextTheme,
    WorkbenchSelectCodeTab,
    WorkbenchSelectBytecodeTab,
    WorkbenchSelectCfgTab,
    WorkbenchSelectCallGraphTab,
    WorkbenchSelectTypeGraphTab,
}

pub fn init_default_keybindings() -> io::Result<()> {
    Ok(())
}

pub fn default_hint() -> String {
    "Editor view: hjkl/arrows navigate, Enter/i edit | Alt-1..5 views | Ctrl-S save | Ctrl-C quit"
        .to_string()
}

pub fn workbench_hint() -> String {
    "Ctrl-W then arrows/hjkl move, e explorer, t tabs, c code, i input, p inspector, 1-5 views, m mode"
        .to_string()
}
