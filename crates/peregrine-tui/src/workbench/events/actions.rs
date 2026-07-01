use crate::workbench::GraphTab;
use crate::workbench::PendingVimCommand;
use crate::workbench::prelude::*;

use crate::keybinds;
use crate::navigation::{self, NavigationCommand};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

impl App {
    pub(crate) fn apply_navigation_command(&mut self, command: NavigationCommand) {
        match command {
            NavigationCommand::Quit => self.exit = Some(WorkbenchExit::Quit),
            NavigationCommand::Save => self.save_current_file(),
            NavigationCommand::Reload => self.reload_current_file(),
            NavigationCommand::Undo => {
                self.editor.undo();
                self.status = String::from("Undo");
            }
            NavigationCommand::BeginWorkbenchNavigation => {
                self.status = keybinds::workbench_hint();
            }
            NavigationCommand::CancelWorkbenchNavigation => {
                self.status = String::from(navigation::WORKBENCH_CANCELED);
            }
            NavigationCommand::UnboundWorkbenchNavigation => {
                self.status = String::from(navigation::WORKBENCH_UNBOUND);
            }
            NavigationCommand::ToggleEditorMode => self.toggle_editor_mode(),
            NavigationCommand::PreviousTheme => self.previous_theme(),
            NavigationCommand::NextTheme => self.next_theme(),
            NavigationCommand::Focus(FocusPane::FileTabs) => {
                self.active_tab = WorkbenchTab::Editor;
                self.set_focus(FocusPane::FileTabs);
            }
            NavigationCommand::Focus(pane) => self.set_focus(pane),
            NavigationCommand::FocusCodeEditor => self.focus_code_editor(),
            NavigationCommand::SwitchToAgent => {
                self.mode = AppMode::Agent;
                self.status = String::from("Switching to agent mode");
                self.exit = Some(WorkbenchExit::SwitchToAgent);
            }
            NavigationCommand::FocusNext => self.set_focus(self.next_focus_pane()),
            NavigationCommand::FocusPrevious => {
                self.set_focus(self.previous_focus_pane());
            }
            NavigationCommand::MoveFocus(direction) => {
                self.set_focus(self.move_focus_pane(direction));
            }
            NavigationCommand::SelectTab(tab) => {
                self.set_active_tab(tab);
            }
        }
    }

    pub(crate) fn handle_explorer_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.explorer.select_previous(),
            KeyCode::Down => self.explorer.select_next(),
            KeyCode::Right => match self.explorer.activate_selected() {
                ExplorerAction::OpenFile(path) => self.open_file(path),
                ExplorerAction::ToggledDirectory => {
                    self.status = String::from("Directory tree updated");
                }
                ExplorerAction::None => {}
            },
            KeyCode::Enter => match self.explorer.activate_selected() {
                ExplorerAction::OpenFile(path) => self.open_file(path),
                ExplorerAction::ToggledDirectory => {
                    self.status = String::from("Directory tree updated");
                }
                ExplorerAction::None => {}
            },
            _ => {}
        }
    }

    pub(crate) fn handle_tabs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.previous_tab(),
            KeyCode::Right | KeyCode::Char('l') => self.next_tab(),
            KeyCode::Down | KeyCode::Enter | KeyCode::Esc => {
                self.set_focus(if self.active_tab == WorkbenchTab::Editor {
                    FocusPane::FileTabs
                } else {
                    FocusPane::Editor
                });
            }
            _ => {}
        }
    }

    pub(crate) fn handle_editor_key(&mut self, key: KeyEvent) {
        match self.active_tab {
            WorkbenchTab::Editor => {
                match self.editor_mode {
                    EditorMode::Standard => self.handle_standard_editor_key(key),
                    EditorMode::Vim => {
                        if self.vim_state == VimState::Insert {
                            if key.code == KeyCode::Esc {
                                self.vim_state = VimState::Normal;
                                self.invalidate_editor_render();
                            } else {
                                self.editor.handle_standard_key(key);
                            }
                        } else {
                            self.handle_vim_normal_key(key);
                        }
                    }
                }
                if self.editor.dirty {
                    self.invalidate_workbench_views();
                }
            }
            WorkbenchTab::Bytecode => self.handle_bytecode_key(key),
            WorkbenchTab::Graphs => self.handle_graph_key(key),
            WorkbenchTab::Chat => {
                let action = self.chat.handle_key(&self.explorer.root, key);
                self.apply_chat_action(action);
            }
        }
    }

    pub(crate) fn handle_standard_editor_key(&mut self, key: KeyEvent) {
        if self.standard_editor_editing {
            if key.code == KeyCode::Esc {
                self.standard_editor_editing = false;
                self.invalidate_editor_render();
                self.status = String::from("Editor navigation: Enter or i to edit");
            } else {
                self.editor.handle_standard_key(key);
            }
            return;
        }

        let plain = key.modifiers == KeyModifiers::NONE;
        match key.code {
            KeyCode::Enter | KeyCode::Char('i') if plain => self.enter_standard_editor_editing(),
            KeyCode::Left | KeyCode::Char('h') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Left,
            )),
            KeyCode::Down | KeyCode::Char('j') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Down,
            )),
            KeyCode::Up | KeyCode::Char('k') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Up,
            )),
            KeyCode::Right | KeyCode::Char('l') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Right,
            )),
            KeyCode::Char('e') if plain => self.set_focus(FocusPane::Explorer),
            KeyCode::Char('t') if plain => self.set_focus(FocusPane::Tabs),
            KeyCode::Char('c') if plain => self.focus_code_editor(),
            KeyCode::Char('1') if plain => self.set_active_tab(WorkbenchTab::Chat),
            KeyCode::Char('2') if plain => self.set_active_tab(WorkbenchTab::Editor),
            KeyCode::Char('3') if plain => self.set_active_tab(WorkbenchTab::Bytecode),
            KeyCode::Char('4') if plain => self.set_active_tab(WorkbenchTab::Graphs),
            KeyCode::PageUp if plain => self.editor.page_up(),
            KeyCode::PageDown if plain => self.editor.page_down(),
            KeyCode::Home if plain => self.editor.move_line_start(),
            KeyCode::End if plain => self.editor.move_line_end(),
            _ => {}
        }
    }

    pub(crate) fn enter_standard_editor_editing(&mut self) {
        self.standard_editor_editing = true;
        self.invalidate_editor_render();
        self.status = String::from("Editing: Esc returns to navigation");
    }

    pub(crate) fn handle_bytecode_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.active_tab = WorkbenchTab::Editor;
            self.standard_editor_editing = false;
            self.set_focus(FocusPane::Editor);
            self.status = String::from("Closed bytecode viewer");
            return;
        }

        let mut request = None;
        let mut show_selector = false;
        let mut load_bytecode = false;

        match &mut self.bytecode {
            BytecodePane::Selecting(selector) => {
                if key.code == KeyCode::Enter {
                    request = selector.selected_request();
                } else {
                    selector.handle_key(key);
                }
            }
            BytecodePane::Ready(session) => {
                if key.code == KeyCode::Enter {
                    show_selector = true;
                } else {
                    session.handle_key(key);
                }
            }
            BytecodePane::Empty | BytecodePane::Message(_) => {
                if key.code == KeyCode::Enter {
                    load_bytecode = true;
                }
            }
            BytecodePane::Loading(_) => {
                if key.code == KeyCode::Enter {
                    self.status = String::from("Bytecode is already loading");
                }
            }
        }

        if load_bytecode {
            self.ensure_bytecode_session();
        }

        if show_selector {
            self.show_bytecode_selector();
        }

        if let Some(request) = request {
            self.load_bytecode_request(request);
        }
    }

    pub(crate) fn handle_graph_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('[') {
            self.prev_graph_tab();
            return;
        }
        if key.code == KeyCode::Char(']') {
            self.next_graph_tab();
            return;
        }

        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            let title = self.active_tab.title();
            self.active_tab = WorkbenchTab::Editor;
            self.standard_editor_editing = false;
            self.set_focus(FocusPane::Editor);
            self.status = format!("Closed {title} viewer");
            return;
        }

        if key.code == KeyCode::Enter {
            self.ensure_graph_tab(self.graphs.active_tab);
            return;
        }

        let Some(GraphPane::Ready(document)) = self.graphs.get_mut(self.graphs.active_tab) else {
            return;
        };
        document.handle_key(key);
    }

    pub(crate) fn handle_vim_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.vim_state = VimState::Normal,
            KeyCode::Char('h') => self.editor.move_left(),
            KeyCode::Char('j') => self.editor.move_down(),
            KeyCode::Char('k') => self.editor.move_up(),
            KeyCode::Char('l') => self.editor.move_right(),
            KeyCode::Char('i') => {
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('a') => {
                self.editor.move_right();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('A') => {
                self.editor.move_line_end();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('I') => {
                self.editor.move_line_start();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('o') => {
                self.editor.open_line_below();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('O') => {
                self.editor.open_line_above();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('x') => self.editor.delete_char(),
            KeyCode::Char('u') => self.editor.undo(),
            KeyCode::Char('p') => self.editor.paste_after(),
            KeyCode::Char('d') => {
                if self.editor.pending_vim == Some(PendingVimCommand::Delete) {
                    self.editor.delete_current_line();
                    self.editor.pending_vim = None;
                } else {
                    self.editor.pending_vim = Some(PendingVimCommand::Delete);
                }
            }
            KeyCode::Char('y') => {
                if self.editor.pending_vim == Some(PendingVimCommand::Yank) {
                    self.editor.yank_current_line();
                    self.editor.pending_vim = None;
                } else {
                    self.editor.pending_vim = Some(PendingVimCommand::Yank);
                }
            }
            _ => {
                self.editor.pending_vim = None;
            }
        }
    }

    pub(crate) fn focus_code_editor(&mut self) {
        self.standard_editor_editing = false;
        self.active_tab = WorkbenchTab::Editor;
        self.set_focus(FocusPane::Editor);
    }

    pub(crate) fn next_graph_tab(&mut self) {
        let index = self.graphs.active_tab.index();
        self.graphs.active_tab = GraphTab::ALL[(index + 1) % GraphTab::ALL.len()];
    }

    pub(crate) fn prev_graph_tab(&mut self) {
        let index = self.graphs.active_tab.index();
        self.graphs.active_tab =
            GraphTab::ALL[(index + GraphTab::ALL.len() - 1) % GraphTab::ALL.len()];
    }

    pub(crate) fn set_active_tab(&mut self, tab: WorkbenchTab) {
        self.active_tab = tab;
        if tab != WorkbenchTab::Editor {
            self.standard_editor_editing = false;
        }
        if tab == WorkbenchTab::Chat {
            self.focus = FocusPane::Input;
            let action = self.chat.tick(&self.explorer.root);
            self.apply_chat_action(action);
            return;
        }
        self.set_focus(self.focus);
    }

    pub(crate) fn set_focus(&mut self, pane: FocusPane) {
        let focus = match pane {
            FocusPane::Input if self.active_tab != WorkbenchTab::Chat => FocusPane::Editor,
            FocusPane::FileTabs if self.active_tab != WorkbenchTab::Editor => FocusPane::Editor,
            other => other,
        };
        if focus != FocusPane::Editor || self.active_tab != WorkbenchTab::Editor {
            self.standard_editor_editing = false;
        }
        self.focus = focus;
    }

    pub(crate) fn next_tab(&mut self) {
        let index = self.active_tab.index();
        self.set_active_tab(WorkbenchTab::ALL[(index + 1) % WorkbenchTab::ALL.len()]);
    }

    pub(crate) fn previous_tab(&mut self) {
        let index = self.active_tab.index();
        self.set_active_tab(
            WorkbenchTab::ALL[(index + WorkbenchTab::ALL.len() - 1) % WorkbenchTab::ALL.len()],
        );
    }

    pub(crate) fn toggle_editor_mode(&mut self) {
        self.editor_mode = match self.editor_mode {
            EditorMode::Standard => EditorMode::Vim,
            EditorMode::Vim => EditorMode::Standard,
        };
        self.standard_editor_editing = false;
        self.vim_state = VimState::Normal;
        self.invalidate_editor_render();
        self.status = format!("Editor mode: {}", self.editor_mode_label());
    }

    pub(crate) fn previous_theme(&mut self) {
        self.theme.previous();
        self.sync_syntax_theme();
        self.invalidate_editor_render();
        self.status = format!("Theme: {}", self.theme.current_name());
    }

    pub(crate) fn next_theme(&mut self) {
        self.theme.next();
        self.sync_syntax_theme();
        self.invalidate_editor_render();
        self.status = format!("Theme: {}", self.theme.current_name());
    }

    pub(crate) fn sync_syntax_theme(&self) {
        if let Some(theme) =
            crate::agent::resolve_theme_by_name(self.theme.current_name().slug(), None)
        {
            crate::agent::set_syntax_theme(theme);
        }
    }

    pub(crate) fn refresh_shared_theme(&mut self) {
        let generation = self.theme.generation();
        if generation != self.theme_generation {
            self.theme_generation = generation;
            self.sync_syntax_theme();
            self.invalidate_workbench_views();
        }
    }

    pub(crate) fn editor_mode_label(&self) -> &'static str {
        match self.editor_mode {
            EditorMode::Standard => {
                if self.standard_editor_editing {
                    "standard edit"
                } else {
                    "standard view"
                }
            }
            EditorMode::Vim => match self.vim_state {
                VimState::Normal => "vim normal",
                VimState::Insert => "vim insert",
            },
        }
    }

    pub(crate) fn open_file(&mut self, path: PathBuf) {
        let interaction = self.current_document_interaction();
        match self.editor.open_file(&path, interaction) {
            Ok(activation) => {
                self.apply_document_interaction(activation.interaction);
                self.invalidate_workbench_views();
                self.active_tab = WorkbenchTab::Editor;
                self.set_focus(FocusPane::Editor);
                self.status = if activation.opened {
                    format!("Opened {}", path.display())
                } else {
                    format!("Activated {}", path.display())
                };
            }
            Err(error) => {
                self.status = format!("Could not open {}: {error}", path.display());
            }
        }
    }

    pub(crate) fn save_current_file(&mut self) {
        match self.editor.save() {
            Ok(()) => {
                self.invalidate_workbench_views();
                self.status = String::from("Saved");
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    pub(crate) fn reload_current_file(&mut self) {
        match self.editor.reload() {
            Ok(()) => {
                self.invalidate_workbench_views();
                self.status = String::from("Reloaded");
            }
            Err(error) => self.status = format!("Reload failed: {error}"),
        }
    }

    pub(crate) fn invalidate_workbench_views(&mut self) {
        self.invalidate_editor_render();
        self.invalidate_bytecode();
        self.invalidate_graphs();
    }

    pub(crate) fn invalidate_editor_render(&mut self) {
        self.editor_render_cache = None;
    }

    pub(crate) fn invalidate_bytecode(&mut self) {
        self.bytecode.invalidate();
        self.bytecode_cache.clear();
        self.bytecode_loader_rx = None;
        self.bytecode_load_epoch = self.bytecode_load_epoch.wrapping_add(1);
    }

    pub(crate) fn invalidate_graphs(&mut self) {
        self.graph_loader_rx = None;
        self.graphs.invalidate();
    }
}
