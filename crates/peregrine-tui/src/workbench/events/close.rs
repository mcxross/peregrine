use crate::workbench::prelude::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(crate) fn request_close_document(&mut self, document_id: DocumentId) {
        if self.editor.document_is_dirty(document_id) {
            self.pending_close = Some(CloseConfirmation {
                document_id,
                selected: CloseChoice::Save,
                error: None,
            });
            return;
        }
        self.close_document(document_id);
    }

    pub(crate) fn handle_close_confirmation_key(&mut self, key: KeyEvent) {
        let Some(confirmation) = self.pending_close.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => {
                confirmation.selected = confirmation.selected.previous();
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => {
                confirmation.selected = confirmation.selected.next();
            }
            KeyCode::Char('s' | 'S') => self.resolve_close_choice(CloseChoice::Save),
            KeyCode::Char('d' | 'D') => self.resolve_close_choice(CloseChoice::Discard),
            KeyCode::Char('c' | 'C') | KeyCode::Esc => {
                self.resolve_close_choice(CloseChoice::Cancel);
            }
            KeyCode::Enter => {
                let selected = confirmation.selected;
                self.resolve_close_choice(selected);
            }
            _ => {}
        }
    }

    pub(crate) fn resolve_close_choice(&mut self, choice: CloseChoice) {
        let Some(document_id) = self
            .pending_close
            .as_ref()
            .map(|confirmation| confirmation.document_id)
        else {
            return;
        };
        match choice {
            CloseChoice::Save => match self.editor.save_document(document_id) {
                Ok(()) => self.close_document(document_id),
                Err(error) => {
                    if let Some(confirmation) = self.pending_close.as_mut() {
                        confirmation.error = Some(format!("Save failed: {error}"));
                    }
                    self.status = format!("Save failed: {error}");
                }
            },
            CloseChoice::Discard => self.close_document(document_id),
            CloseChoice::Cancel => {
                self.pending_close = None;
                self.status = String::from("Close canceled");
            }
        }
    }

    fn close_document(&mut self, document_id: DocumentId) {
        let label = self
            .editor
            .document_label(document_id)
            .unwrap_or("file")
            .to_string();
        let interaction = self.current_document_interaction();
        let Some(result) = self.editor.close(document_id, interaction) else {
            self.pending_close = None;
            return;
        };
        if result.was_active {
            self.apply_document_interaction(result.interaction);
            self.invalidate_workbench_views();
        }
        self.pending_close = None;
        self.status = format!("Closed {label}");
    }
}
