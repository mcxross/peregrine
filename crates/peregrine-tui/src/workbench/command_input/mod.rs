use crate::workbench::{char_len, char_to_byte_index, editable_char_modifiers};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Default, Clone)]
pub struct CommandInput {
    pub(crate) text: String,
    pub(crate) cursor: usize,
}

impl CommandInput {
    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = char_len(&text);
        Self { text, cursor }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if editable_char_modifiers(key.modifiers) => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_char(),
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(char_len(&self.text)),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = char_len(&self.text),
            _ => {}
        }
    }

    fn insert_char(&mut self, c: char) {
        let byte = char_to_byte_index(&self.text, self.cursor);
        self.text.insert(byte, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = char_to_byte_index(&self.text, self.cursor);
        let start = char_to_byte_index(&self.text, self.cursor - 1);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
    }

    fn delete_char(&mut self) {
        if self.cursor >= char_len(&self.text) {
            return;
        }
        let start = char_to_byte_index(&self.text, self.cursor);
        let end = char_to_byte_index(&self.text, self.cursor + 1);
        self.text.replace_range(start..end, "");
    }
}
