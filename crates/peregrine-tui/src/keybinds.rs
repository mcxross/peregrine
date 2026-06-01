use crossterm_keybind::{DisplayFormat, KeyBind, KeyBindTrait};
use std::io;
use std::sync::{Once, OnceLock};

/// Key binding events for the workbench.
///
/// This follows the ratatui-keybind-template pattern: all default bindings live
/// in one derived enum, and the app dispatches semantic events instead of
/// comparing raw terminal keys throughout the codebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, KeyBind)]
pub enum KeyBindEvent {
    /// Quit the application.
    #[keybindings["Control+c", "Control+q"]]
    Quit,

    /// Start a one-key workbench navigation chord.
    #[keybindings["Control+w"]]
    BeginWorkbenchNavigation,

    /// Save the current file.
    #[keybindings["Control+s"]]
    Save,

    /// Reload the current file, discarding unsaved edits.
    #[keybindings["Control+r"]]
    Reload,

    /// Undo the last editor operation.
    #[keybindings["Control+z"]]
    Undo,

    /// Move focus to the next screen section.
    #[keybindings["Tab"]]
    FocusNext,

    /// Move focus to the previous screen section.
    #[keybindings["BackTab"]]
    FocusPrevious,

    /// Select the code tab.
    #[keybindings["Alternate+1"]]
    SelectCodeTab,

    /// Select the bytecode tab.
    #[keybindings["Alternate+2"]]
    SelectBytecodeTab,

    /// Select the CFG tab.
    #[keybindings["Alternate+3"]]
    SelectCfgTab,

    /// Select the call graph tab.
    #[keybindings["Alternate+4"]]
    SelectCallGraphTab,

    /// Select the type graph tab.
    #[keybindings["Alternate+5"]]
    SelectTypeGraphTab,

    /// Cancel the pending workbench navigation chord.
    #[keybindings["Esc"]]
    WorkbenchCancel,

    /// Move workbench focus left.
    #[keybindings["h", "Left"]]
    WorkbenchFocusLeft,

    /// Move workbench focus down.
    #[keybindings["j", "Down"]]
    WorkbenchFocusDown,

    /// Move workbench focus up.
    #[keybindings["k", "Up"]]
    WorkbenchFocusUp,

    /// Move workbench focus right.
    #[keybindings["l", "Right"]]
    WorkbenchFocusRight,

    /// Focus the explorer pane.
    #[keybindings["e"]]
    WorkbenchFocusExplorer,

    /// Focus the tab bar.
    #[keybindings["t"]]
    WorkbenchFocusTabs,

    /// Focus the code editor.
    #[keybindings["c"]]
    WorkbenchFocusCodeEditor,

    /// Focus the bottom input field.
    #[keybindings["i"]]
    WorkbenchFocusInput,

    /// Focus the inspector pane.
    #[keybindings["p"]]
    WorkbenchFocusInspector,

    /// Toggle standard/vim-like editor mode.
    #[keybindings["m"]]
    WorkbenchToggleEditorMode,

    /// Cycle to the previous color theme.
    #[keybindings["["]]
    WorkbenchPreviousTheme,

    /// Cycle to the next color theme.
    #[keybindings["]"]]
    WorkbenchNextTheme,

    /// Select the code tab from workbench navigation.
    #[keybindings["1"]]
    WorkbenchSelectCodeTab,

    /// Select the bytecode tab from workbench navigation.
    #[keybindings["2"]]
    WorkbenchSelectBytecodeTab,

    /// Select the CFG tab from workbench navigation.
    #[keybindings["3"]]
    WorkbenchSelectCfgTab,

    /// Select the call graph tab from workbench navigation.
    #[keybindings["4"]]
    WorkbenchSelectCallGraphTab,

    /// Select the type graph tab from workbench navigation.
    #[keybindings["5"]]
    WorkbenchSelectTypeGraphTab,
}

pub fn init_default_keybindings() -> io::Result<()> {
    static INIT: Once = Once::new();
    static ERROR: OnceLock<String> = OnceLock::new();

    INIT.call_once(|| {
        if let Err(error) = KeyBindEvent::init_and_load(None) {
            let _ = ERROR.set(error.to_string());
        }
    });

    if let Some(error) = ERROR.get() {
        Err(io::Error::other(error.clone()))
    } else {
        Ok(())
    }
}

pub fn default_hint() -> String {
    let format = DisplayFormat::Abbreviation;
    format!(
        "{} quit | {} workbench nav | {} save | {} reload | {} undo",
        KeyBindEvent::Quit.key_bindings_display_with_format(&format),
        KeyBindEvent::BeginWorkbenchNavigation.key_bindings_display_with_format(&format),
        KeyBindEvent::Save.key_bindings_display_with_format(&format),
        KeyBindEvent::Reload.key_bindings_display_with_format(&format),
        KeyBindEvent::Undo.key_bindings_display_with_format(&format),
    )
}

pub fn workbench_hint() -> String {
    let format = DisplayFormat::Abbreviation;
    format!(
        "{}: hjkl move, {} explorer, {} tabs, {} code, {} input, {} inspector, 1-5 tabs, {} mode, {}/{} theme, {} cancel",
        KeyBindEvent::BeginWorkbenchNavigation.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchFocusExplorer.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchFocusTabs.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchFocusCodeEditor.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchFocusInput.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchFocusInspector.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchToggleEditorMode.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchPreviousTheme.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchNextTheme.key_bindings_display_with_format(&format),
        KeyBindEvent::WorkbenchCancel.key_bindings_display_with_format(&format),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybind_template_generates_toml_example() {
        let toml = KeyBindEvent::toml_example();

        assert!(toml.contains("quit"));
        assert!(toml.contains("begin_workbench_navigation"));
        assert!(toml.contains("workbench_focus_code_editor"));
        assert!(toml.contains("workbench_next_theme"));
    }
}
