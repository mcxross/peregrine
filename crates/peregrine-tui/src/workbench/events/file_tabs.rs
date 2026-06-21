use crate::navigation;
use crate::workbench::prelude::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(crate) fn handle_file_tabs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.select_previous_document(),
            KeyCode::Right | KeyCode::Char('l') => self.select_next_document(),
            KeyCode::Home => self.select_first_document(),
            KeyCode::End => self.select_last_document(),
            KeyCode::Up => self.set_focus(FocusPane::Tabs),
            KeyCode::Down | KeyCode::Enter | KeyCode::Esc => self.set_focus(FocusPane::Editor),
            KeyCode::Delete => {
                if let Some(id) = self.editor.active_id() {
                    self.request_close_document(id);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn next_focus_pane(&self) -> FocusPane {
        if self.active_tab == WorkbenchTab::Editor {
            navigation::next_focus(self.focus)
        } else {
            match self.focus {
                FocusPane::Explorer => FocusPane::Editor,
                FocusPane::Editor | FocusPane::FileTabs => FocusPane::Tabs,
                FocusPane::Tabs | FocusPane::Input => FocusPane::Explorer,
            }
        }
    }

    pub(crate) fn previous_focus_pane(&self) -> FocusPane {
        if self.active_tab == WorkbenchTab::Editor {
            navigation::previous_focus(self.focus)
        } else {
            match self.focus {
                FocusPane::Explorer => FocusPane::Tabs,
                FocusPane::Editor | FocusPane::FileTabs => FocusPane::Explorer,
                FocusPane::Tabs | FocusPane::Input => FocusPane::Editor,
            }
        }
    }

    pub(crate) fn move_focus_pane(&self, direction: navigation::FocusDirection) -> FocusPane {
        if self.active_tab == WorkbenchTab::Editor {
            return navigation::move_focus(self.focus, direction);
        }
        match direction {
            navigation::FocusDirection::Left => match self.focus {
                FocusPane::Explorer => FocusPane::Explorer,
                FocusPane::Tabs | FocusPane::FileTabs | FocusPane::Editor | FocusPane::Input => {
                    FocusPane::Explorer
                }
            },
            navigation::FocusDirection::Right => match self.focus {
                FocusPane::Explorer => FocusPane::Editor,
                FocusPane::Tabs | FocusPane::FileTabs | FocusPane::Editor | FocusPane::Input => {
                    FocusPane::Editor
                }
            },
            navigation::FocusDirection::Up => match self.focus {
                FocusPane::Input => FocusPane::Editor,
                FocusPane::Editor | FocusPane::FileTabs => FocusPane::Tabs,
                other => other,
            },
            navigation::FocusDirection::Down => match self.focus {
                FocusPane::Tabs | FocusPane::FileTabs => FocusPane::Editor,
                FocusPane::Editor => FocusPane::Editor,
                other => other,
            },
        }
    }

    pub(crate) fn activate_document(&mut self, id: DocumentId) {
        let interaction = self
            .editor
            .activate(id, self.current_document_interaction());
        self.apply_document_interaction(interaction);
        self.active_tab = WorkbenchTab::Editor;
        self.invalidate_workbench_views();
    }

    pub(crate) fn select_previous_document(&mut self) {
        let interaction = self
            .editor
            .select_previous(self.current_document_interaction());
        self.apply_document_interaction(interaction);
        self.invalidate_workbench_views();
    }

    pub(crate) fn select_next_document(&mut self) {
        let interaction = self.editor.select_next(self.current_document_interaction());
        self.apply_document_interaction(interaction);
        self.invalidate_workbench_views();
    }

    pub(crate) fn select_first_document(&mut self) {
        let interaction = self
            .editor
            .select_first(self.current_document_interaction());
        self.apply_document_interaction(interaction);
        self.invalidate_workbench_views();
    }

    pub(crate) fn select_last_document(&mut self) {
        let interaction = self.editor.select_last(self.current_document_interaction());
        self.apply_document_interaction(interaction);
        self.invalidate_workbench_views();
    }

    pub(crate) fn current_document_interaction(&self) -> DocumentInteractionState {
        DocumentInteractionState {
            standard_editing: self.standard_editor_editing,
            vim_state: self.vim_state,
        }
    }

    pub(crate) fn apply_document_interaction(&mut self, interaction: DocumentInteractionState) {
        self.standard_editor_editing = interaction.standard_editing;
        self.vim_state = interaction.vim_state;
    }
}
