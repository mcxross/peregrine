use crate::workbench::{char_len, char_to_byte_index, editable_char_modifiers};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub struct CommandInput {
    pub(crate) text: String,
    pub(crate) cursor: usize,
    pub(crate) scroll: usize,
    pub(crate) viewport_width: usize,
}

impl Default for CommandInput {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            scroll: 0,
            viewport_width: 1,
        }
    }
}

impl CommandInput {
    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = char_len(&text);
        let mut input = Self {
            text,
            cursor,
            scroll: 0,
            viewport_width: 1,
        };
        input.ensure_cursor_visible();
        input
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
        self.ensure_cursor_visible();
    }

    pub(crate) fn set_viewport_width(&mut self, width: usize) {
        self.viewport_width = width.max(1);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    pub(crate) fn scroll_horizontal(&mut self, right: bool, amount: usize) {
        if right {
            self.scroll = self.scroll.saturating_add(amount).min(self.max_scroll());
        } else {
            self.scroll = self.scroll.saturating_sub(amount);
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

    pub(crate) fn set_cursor_column(&mut self, col: usize) {
        self.cursor = (self.scroll + col).min(char_len(&self.text));
        self.ensure_cursor_visible();
    }

    pub(crate) fn take_text(&mut self) -> String {
        self.cursor = 0;
        self.scroll = 0;
        std::mem::take(&mut self.text)
    }

    fn ensure_cursor_visible(&mut self) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + self.viewport_width {
            self.scroll = self.cursor + 1 - self.viewport_width;
        }
        self.scroll = self.scroll.min(self.max_scroll());
    }

    fn max_scroll(&self) -> usize {
        char_len(&self.text)
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }
}
