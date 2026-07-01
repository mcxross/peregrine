use crate::keybinds::KeyBindEvent;
use crate::{FocusPane, WorkbenchTab};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub const WORKBENCH_CANCELED: &str = "Workbench navigation canceled";
pub const WORKBENCH_UNBOUND: &str = "Workbench navigation key is not bound";

const FOCUS_ORDER: [FocusPane; 4] = [
    FocusPane::Explorer,
    FocusPane::FileTabs,
    FocusPane::Editor,
    FocusPane::Tabs,
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

    pub fn translate_workbench_navigation_only(&mut self, key: KeyEvent) -> NavigationIntent {
        if let Some(chord) = self.pending_chord.take() {
            return NavigationIntent::Command(chord_command(chord, key));
        }

        for event in dispatch_key(key) {
            let command = match event {
                KeyBindEvent::BeginWorkbenchNavigation => {
                    self.pending_chord = Some(NavigationChord::Workbench);
                    NavigationCommand::BeginWorkbenchNavigation
                }
                KeyBindEvent::SelectCodeTab => NavigationCommand::SelectTab(WorkbenchTab::Editor),
                KeyBindEvent::SelectBytecodeTab => {
                    NavigationCommand::SelectTab(WorkbenchTab::Bytecode)
                }
                KeyBindEvent::SelectCfgTab => NavigationCommand::SelectTab(WorkbenchTab::Graphs),
                KeyBindEvent::SelectCallGraphTab => {
                    NavigationCommand::SelectTab(WorkbenchTab::Graphs)
                }
                KeyBindEvent::SelectTypeGraphTab => {
                    NavigationCommand::SelectTab(WorkbenchTab::Graphs)
                }
                KeyBindEvent::SelectChatTab => NavigationCommand::SelectTab(WorkbenchTab::Chat),
                _ => continue,
            };
            return NavigationIntent::Command(command);
        }

        NavigationIntent::PassThrough
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
    SwitchToAgent,
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
            FocusPane::Tabs | FocusPane::FileTabs | FocusPane::Editor | FocusPane::Input => {
                FocusPane::Explorer
            }
            FocusPane::Explorer => FocusPane::Explorer,
        },
        FocusDirection::Right => match current {
            FocusPane::Explorer => FocusPane::Editor,
            FocusPane::Tabs | FocusPane::FileTabs | FocusPane::Editor | FocusPane::Input => {
                FocusPane::Editor
            }
        },
        FocusDirection::Up => match current {
            FocusPane::Input => FocusPane::Editor,
            FocusPane::Editor => FocusPane::FileTabs,
            FocusPane::FileTabs => FocusPane::Tabs,
            other => other,
        },
        FocusDirection::Down => match current {
            FocusPane::Tabs => FocusPane::FileTabs,
            FocusPane::FileTabs => FocusPane::Editor,
            FocusPane::Editor => FocusPane::Editor,
            other => other,
        },
    }
}

fn always_available_command(key: KeyEvent) -> Option<NavigationCommand> {
    for event in dispatch_key(key) {
        if event == KeyBindEvent::Quit {
            return Some(NavigationCommand::Quit);
        }
    }
    None
}

fn global_command(key: KeyEvent, focus: FocusPane) -> Option<NavigationCommand> {
    for event in dispatch_key(key) {
        let command = match event {
            KeyBindEvent::BeginWorkbenchNavigation => NavigationCommand::BeginWorkbenchNavigation,
            KeyBindEvent::Save => NavigationCommand::Save,
            KeyBindEvent::Reload => NavigationCommand::Reload,
            KeyBindEvent::Undo => NavigationCommand::Undo,
            KeyBindEvent::FocusNext if focus != FocusPane::Editor => NavigationCommand::FocusNext,
            KeyBindEvent::FocusPrevious => NavigationCommand::FocusPrevious,
            KeyBindEvent::SelectCodeTab => NavigationCommand::SelectTab(WorkbenchTab::Editor),
            KeyBindEvent::SelectBytecodeTab => NavigationCommand::SelectTab(WorkbenchTab::Bytecode),
            KeyBindEvent::SelectCfgTab => NavigationCommand::SelectTab(WorkbenchTab::Graphs),
            KeyBindEvent::SelectCallGraphTab => NavigationCommand::SelectTab(WorkbenchTab::Graphs),
            KeyBindEvent::SelectTypeGraphTab => NavigationCommand::SelectTab(WorkbenchTab::Graphs),
            KeyBindEvent::SelectChatTab => NavigationCommand::SelectTab(WorkbenchTab::Chat),
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
    for event in dispatch_key(key) {
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
            KeyBindEvent::WorkbenchFocusFileTabs => NavigationCommand::Focus(FocusPane::FileTabs),
            KeyBindEvent::WorkbenchFocusCodeEditor => NavigationCommand::FocusCodeEditor,
            KeyBindEvent::WorkbenchSwitchToAgent => NavigationCommand::SwitchToAgent,
            KeyBindEvent::WorkbenchToggleEditorMode => NavigationCommand::ToggleEditorMode,
            KeyBindEvent::WorkbenchPreviousTheme => NavigationCommand::PreviousTheme,
            KeyBindEvent::WorkbenchNextTheme => NavigationCommand::NextTheme,
            KeyBindEvent::WorkbenchSelectCodeTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Editor)
            }
            KeyBindEvent::WorkbenchSelectBytecodeTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Bytecode)
            }
            KeyBindEvent::WorkbenchSelectCfgTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Graphs)
            }
            KeyBindEvent::WorkbenchSelectCallGraphTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Graphs)
            }
            KeyBindEvent::WorkbenchSelectTypeGraphTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Graphs)
            }
            KeyBindEvent::WorkbenchSelectChatTab => {
                NavigationCommand::SelectTab(WorkbenchTab::Chat)
            }
            KeyBindEvent::FocusNext => NavigationCommand::FocusNext,
            KeyBindEvent::FocusPrevious => NavigationCommand::FocusPrevious,
            _ => continue,
        };
        return command;
    }
    NavigationCommand::UnboundWorkbenchNavigation
}

fn dispatch_key(key: KeyEvent) -> Vec<KeyBindEvent> {
    let mut events = Vec::new();
    let modifiers = key.modifiers;
    let code = key.code;

    match (code, modifiers) {
        (KeyCode::Char('c' | 'C'), KeyModifiers::CONTROL)
        | (KeyCode::Char('q' | 'Q'), KeyModifiers::CONTROL) => events.push(KeyBindEvent::Quit),
        (KeyCode::Char('w' | 'W'), KeyModifiers::CONTROL) => {
            events.push(KeyBindEvent::BeginWorkbenchNavigation)
        }
        (KeyCode::Char('s' | 'S'), KeyModifiers::CONTROL) => events.push(KeyBindEvent::Save),
        (KeyCode::Char('r' | 'R'), KeyModifiers::CONTROL) => events.push(KeyBindEvent::Reload),
        (KeyCode::Char('z' | 'Z'), KeyModifiers::CONTROL) => events.push(KeyBindEvent::Undo),
        (KeyCode::Tab, KeyModifiers::NONE) => events.push(KeyBindEvent::FocusNext),
        (KeyCode::BackTab, _) => events.push(KeyBindEvent::FocusPrevious),
        (KeyCode::Char('1'), KeyModifiers::ALT) => events.push(KeyBindEvent::SelectCodeTab),
        (KeyCode::Char('2'), KeyModifiers::ALT) => events.push(KeyBindEvent::SelectBytecodeTab),
        (KeyCode::Char('3'), KeyModifiers::ALT) => events.push(KeyBindEvent::SelectCfgTab),
        (KeyCode::Char('4'), KeyModifiers::ALT) => events.push(KeyBindEvent::SelectCallGraphTab),
        (KeyCode::Char('5'), KeyModifiers::ALT) => events.push(KeyBindEvent::SelectTypeGraphTab),
        (KeyCode::Char('6'), KeyModifiers::ALT) => events.push(KeyBindEvent::SelectChatTab),
        (KeyCode::Esc, _) => events.push(KeyBindEvent::WorkbenchCancel),
        (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusLeft)
        }
        (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusDown)
        }
        (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusUp)
        }
        (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusRight)
        }
        (KeyCode::Char('e'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusExplorer)
        }
        (KeyCode::Char('t'), KeyModifiers::NONE) => events.push(KeyBindEvent::WorkbenchFocusTabs),
        (KeyCode::Char('f'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusFileTabs)
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchFocusCodeEditor)
        }
        (KeyCode::Char('a'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSwitchToAgent)
        }
        (KeyCode::Char('m'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchToggleEditorMode)
        }
        (KeyCode::Char('['), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchPreviousTheme)
        }
        (KeyCode::Char(']'), KeyModifiers::NONE) => events.push(KeyBindEvent::WorkbenchNextTheme),
        (KeyCode::Char('1'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSelectCodeTab)
        }
        (KeyCode::Char('2'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSelectBytecodeTab)
        }
        (KeyCode::Char('3'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSelectCfgTab)
        }
        (KeyCode::Char('4'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSelectCallGraphTab)
        }
        (KeyCode::Char('5'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSelectTypeGraphTab)
        }
        (KeyCode::Char('6'), KeyModifiers::NONE) => {
            events.push(KeyBindEvent::WorkbenchSelectChatTab)
        }
        _ => {}
    }

    events
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
        let second = navigation.translate(key(KeyCode::Char('a')), FocusPane::Editor);

        assert_eq!(
            first,
            NavigationIntent::Command(NavigationCommand::BeginWorkbenchNavigation)
        );
        assert_eq!(
            second,
            NavigationIntent::Command(NavigationCommand::SwitchToAgent)
        );
    }

    #[test]
    fn workbench_chord_maps_six_to_chat_tab() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();
        let first = navigation.translate(ctrl('w'), FocusPane::Editor);
        let second = navigation.translate(key(KeyCode::Char('6')), FocusPane::Editor);

        assert_eq!(
            first,
            NavigationIntent::Command(NavigationCommand::BeginWorkbenchNavigation)
        );
        assert_eq!(
            second,
            NavigationIntent::Command(NavigationCommand::SelectTab(WorkbenchTab::Chat))
        );
    }

    #[test]
    fn workbench_chord_focuses_file_tabs() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();
        navigation.translate(ctrl('w'), FocusPane::Editor);

        assert_eq!(
            navigation.translate(key(KeyCode::Char('f')), FocusPane::Editor),
            NavigationIntent::Command(NavigationCommand::Focus(FocusPane::FileTabs))
        );
    }

    #[test]
    fn alt_six_selects_chat_tab() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();

        assert_eq!(
            navigation.translate(alt('6'), FocusPane::Editor),
            NavigationIntent::Command(NavigationCommand::SelectTab(WorkbenchTab::Chat))
        );
    }

    #[test]
    fn chat_tab_navigation_only_keeps_ctrl_c_for_chat() {
        keybinds::init_default_keybindings().expect("keybindings");
        let mut navigation = Navigation::default();

        assert_eq!(
            navigation.translate_workbench_navigation_only(ctrl('c')),
            NavigationIntent::PassThrough
        );
        assert_eq!(
            navigation.translate_workbench_navigation_only(alt('6')),
            NavigationIntent::Command(NavigationCommand::SelectTab(WorkbenchTab::Chat))
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
            FocusPane::Editor
        );
        assert_eq!(
            move_focus(FocusPane::Editor, FocusDirection::Down),
            FocusPane::Editor
        );
        assert_eq!(
            move_focus(FocusPane::Editor, FocusDirection::Right),
            FocusPane::Editor
        );
        assert_eq!(
            move_focus(FocusPane::Editor, FocusDirection::Up),
            FocusPane::FileTabs
        );
        assert_eq!(
            move_focus(FocusPane::FileTabs, FocusDirection::Up),
            FocusPane::Tabs
        );
        assert_eq!(
            move_focus(FocusPane::Tabs, FocusDirection::Down),
            FocusPane::FileTabs
        );
        assert_eq!(
            move_focus(FocusPane::FileTabs, FocusDirection::Down),
            FocusPane::Editor
        );
    }

    #[test]
    fn dispatch_maps_core_workbench_keys() {
        assert_eq!(dispatch_key(ctrl('c')), vec![KeyBindEvent::Quit]);
        assert_eq!(
            dispatch_key(ctrl('w')),
            vec![KeyBindEvent::BeginWorkbenchNavigation]
        );
        assert_eq!(
            dispatch_key(key(KeyCode::Char('a'))),
            vec![KeyBindEvent::WorkbenchSwitchToAgent]
        );
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn alt(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
    }
}
