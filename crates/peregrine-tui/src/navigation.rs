use crate::keybinds::KeyBindEvent;
use crate::{FocusPane, WorkbenchTab};
use crossterm_keybind::KeyBindTrait;
use ratatui::crossterm::event::KeyEvent;

pub const WORKBENCH_CANCELED: &str = "Workbench navigation canceled";
pub const WORKBENCH_UNBOUND: &str = "Workbench navigation key is not bound";

const FOCUS_ORDER: [FocusPane; 5] = [
    FocusPane::Explorer,
    FocusPane::Tabs,
    FocusPane::Editor,
    FocusPane::Input,
    FocusPane::Inspector,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Navigation {
    pending_chord: Option<NavigationChord>,
}

impl Navigation {
    pub fn translate(&mut self, key: KeyEvent, focus: FocusPane) -> NavigationIntent {
        if let Some(command) = always_available_command(key) {
            return NavigationIntent::Command(command);
        }

        if let Some(chord) = self.pending_chord.take() {
            return NavigationIntent::Command(chord_command(chord, key));
        }

        if let Some(command) = global_command(key, focus) {
            if command == NavigationCommand::BeginWorkbenchNavigation {
                self.pending_chord = Some(NavigationChord::Workbench);
            }
            NavigationIntent::Command(command)
        } else {
            NavigationIntent::PassThrough
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationChord {
    Workbench,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationIntent {
    Command(NavigationCommand),
    PassThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationCommand {
    Quit,
    Save,
    Reload,
    Undo,
    BeginWorkbenchNavigation,
    CancelWorkbenchNavigation,
    UnboundWorkbenchNavigation,
    ToggleEditorMode,
    PreviousTheme,
    NextTheme,
    Focus(FocusPane),
    FocusCodeEditor,
    FocusNext,
    FocusPrevious,
    MoveFocus(FocusDirection),
    SelectTab(WorkbenchTab),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Left,
    Down,
    Up,
    Right,
}

pub fn next_focus(current: FocusPane) -> FocusPane {
    let index = FOCUS_ORDER
        .iter()
        .position(|pane| *pane == current)
        .unwrap_or_default();
    FOCUS_ORDER[(index + 1) % FOCUS_ORDER.len()]
}

pub fn previous_focus(current: FocusPane) -> FocusPane {
    let index = FOCUS_ORDER
        .iter()
        .position(|pane| *pane == current)
        .unwrap_or_default();
    FOCUS_ORDER[(index + FOCUS_ORDER.len() - 1) % FOCUS_ORDER.len()]
}

pub fn move_focus(current: FocusPane, direction: FocusDirection) -> FocusPane {
    match direction {
        FocusDirection::Left => match current {
            FocusPane::Inspector => FocusPane::Editor,
            FocusPane::Tabs | FocusPane::Editor | FocusPane::Input => FocusPane::Explorer,
            FocusPane::Explorer => FocusPane::Explorer,
        },
        FocusDirection::Right => match current {
            FocusPane::Explorer => FocusPane::Tabs,
            FocusPane::Tabs | FocusPane::Editor | FocusPane::Input => FocusPane::Inspector,
            FocusPane::Inspector => FocusPane::Inspector,
        },
        FocusDirection::Up => match current {
            FocusPane::Input => FocusPane::Editor,
            FocusPane::Editor => FocusPane::Tabs,
            other => other,
        },
        FocusDirection::Down => match current {
            FocusPane::Tabs => FocusPane::Editor,
            FocusPane::Editor => FocusPane::Input,
            other => other,
        },
    }
}

fn always_available_command(key: KeyEvent) -> Option<NavigationCommand> {
    for event in KeyBindEvent::dispatch(&key) {
        if event == KeyBindEvent::Quit {
            return Some(NavigationCommand::Quit);
        }
    }
    None
}

fn global_command(key: KeyEvent, focus: FocusPane) -> Option<NavigationCommand> {
    for event in KeyBindEvent::dispatch(&key) {
        let command = match event {
            KeyBindEvent::BeginWorkbenchNavigation => NavigationCommand::BeginWorkbenchNavigation,
            KeyBindEvent::Save => NavigationCommand::Save,
            KeyBindEvent::Reload => NavigationCommand::Reload,
            KeyBindEvent::Undo => NavigationCommand::Undo,
            KeyBindEvent::FocusNext if focus != FocusPane::Editor => NavigationCommand::FocusNext,
            KeyBindEvent::FocusPrevious => NavigationCommand::FocusPrevious,
            KeyBindEvent::SelectCodeTab => NavigationCommand::SelectTab(WorkbenchTab::Code),
            KeyBindEvent::SelectBytecodeTab => NavigationCommand::SelectTab(WorkbenchTab::Bytecode),
            KeyBindEvent::SelectCfgTab => NavigationCommand::SelectTab(WorkbenchTab::Cfg),
            KeyBindEvent::SelectCallGraphTab => {
                NavigationCommand::SelectTab(WorkbenchTab::CallGraph)
            }
            KeyBindEvent::SelectTypeGraphTab => {
                NavigationCommand::SelectTab(WorkbenchTab::TypeGraph)
            }
            _ => continue,
        };
        return Some(command);
    }
    None
}

fn chord_command(chord: NavigationChord, key: KeyEvent) -> NavigationCommand {
    match chord {
        NavigationChord::Workbench => workbench_command(key),
    }
}

fn workbench_command(key: KeyEvent) -> NavigationCommand {
    for event in KeyBindEvent::dispatch(&key) {
        let command = match event {
            KeyBindEvent::WorkbenchCancel => NavigationCommand::CancelWorkbenchNavigation,
            KeyBindEvent::WorkbenchFocusLeft => NavigationCommand::MoveFocus(FocusDirection::Left),
            KeyBindEvent::WorkbenchFocusDown => NavigationCommand::MoveFocus(FocusDirection::Down),
            KeyBindEvent::WorkbenchFocusUp => NavigationCommand::MoveFocus(FocusDirection::Up),
            KeyBindEvent::WorkbenchFocusRight => {
                NavigationCommand::MoveFocus(FocusDirection::Right)
            }
            KeyBindEvent::WorkbenchFocusExplorer => NavigationCommand::Focus(FocusPane::Explorer),
            KeyBindEvent::WorkbenchFocusTabs => NavigationCommand::Focus(FocusPane::Tabs),
            KeyBindEvent::WorkbenchFocusCodeEditor => NavigationCommand::FocusCodeEditor,
            KeyBindEvent::WorkbenchFocusInput => NavigationCommand::Focus(FocusPane::Input),
            KeyBindEvent::WorkbenchFocusInspector => NavigationCommand::Focus(FocusPane::Inspector),
            KeyBindEvent::WorkbenchToggleEditorMode => NavigationCommand::ToggleEditorMode,
            KeyBindEvent::WorkbenchPreviousTheme => NavigationCommand::PreviousTheme,
            KeyBindEvent::WorkbenchNextTheme => NavigationCommand::NextTheme,
            KeyBindEvent::WorkbenchSelectCodeTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Code)
            }
            KeyBindEvent::WorkbenchSelectBytecodeTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Bytecode)
            }
            KeyBindEvent::WorkbenchSelectCfgTab => NavigationCommand::SelectTab(WorkbenchTab::Cfg),
            KeyBindEvent::WorkbenchSelectCallGraphTab => {
                NavigationCommand::SelectTab(WorkbenchTab::CallGraph)
            }
            KeyBindEvent::WorkbenchSelectTypeGraphTab => {
                NavigationCommand::SelectTab(WorkbenchTab::TypeGraph)
            }
            KeyBindEvent::FocusNext => NavigationCommand::FocusNext,
            KeyBindEvent::FocusPrevious => NavigationCommand::FocusPrevious,
            _ => continue,
        };
        return command;
    }
    NavigationCommand::UnboundWorkbenchNavigation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn workbench_chord_maps_following_key_to_command() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();
        let first = navigation.translate(ctrl('w'), FocusPane::Editor);
        let second = navigation.translate(key(KeyCode::Char('p')), FocusPane::Editor);

        assert_eq!(
            first,
            NavigationIntent::Command(NavigationCommand::BeginWorkbenchNavigation)
        );
        assert_eq!(
            second,
            NavigationIntent::Command(NavigationCommand::Focus(FocusPane::Inspector))
        );
    }

    #[test]
    fn quit_preempts_pending_chord() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();
        navigation.translate(ctrl('w'), FocusPane::Editor);

        assert_eq!(
            navigation.translate(ctrl('c'), FocusPane::Editor),
            NavigationIntent::Command(NavigationCommand::Quit)
        );
    }

    #[test]
    fn editor_tab_passes_through_but_other_panes_cycle_focus() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();

        assert_eq!(
            navigation.translate(key(KeyCode::Tab), FocusPane::Editor),
            NavigationIntent::PassThrough
        );
        assert_eq!(
            navigation.translate(key(KeyCode::Tab), FocusPane::Explorer),
            NavigationIntent::Command(NavigationCommand::FocusNext)
        );
    }

    #[test]
    fn focus_graph_is_explicit() {
        assert_eq!(
            move_focus(FocusPane::Explorer, FocusDirection::Right),
            FocusPane::Tabs
        );
        assert_eq!(
            move_focus(FocusPane::Editor, FocusDirection::Down),
            FocusPane::Input
        );
        assert_eq!(
            move_focus(FocusPane::Inspector, FocusDirection::Left),
            FocusPane::Editor
        );
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
}
