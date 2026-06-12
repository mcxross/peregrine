use super::types::{Cursor, EditorSnapshot, PendingVimCommand};
use super::super::{
    char_len, char_to_byte_index, editable_char_modifiers, split_lines, PAGE_SIZE, UNDO_LIMIT,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct EditorBuffer {
    pub(crate) path: Option<PathBuf>,
    pub(crate) lines: Vec<String>,
    pub(crate) cursor: Cursor,
    pub(crate) scroll: usize,
    pub(crate) horizontal_scroll: usize,
    pub(crate) dirty: bool,
    pub(crate) undo_stack: Vec<EditorSnapshot>,
    pub(crate) yank: Vec<String>,
    pub(crate) pending_vim: Option<PendingVimCommand>,
    pub(crate) viewport_height: usize,
    pub(crate) viewport_width: usize,
}

impl EditorBuffer {
    pub fn new_empty() -> Self {
        Self {
            path: None,
            lines: vec![String::new()],
            cursor: Cursor { row: 0, col: 0 },
            scroll: 0,
            horizontal_scroll: 0,
            dirty: false,
            undo_stack: Vec::new(),
            yank: Vec::new(),
            pending_vim: None,
            viewport_height: 1,
            viewport_width: 1,
        }
    }

    pub fn open_file(&mut self, path: &Path) -> io::Result<()> {
        let contents = fs::read_to_string(path)?;
        self.path = Some(path.to_path_buf());
        self.lines = split_lines(&contents);
        self.cursor = Cursor { row: 0, col: 0 };
        self.scroll = 0;
        self.horizontal_scroll = 0;
        self.dirty = false;
        self.undo_stack.clear();
        self.pending_vim = None;
        Ok(())
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no file is open",
            ));
        };
        fs::write(path, self.text())?;
        self.dirty = false;
        Ok(())
    }

    pub fn reload(&mut self) -> io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no file is open",
            ));
        };
        self.open_file(&path)
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    pub(crate) fn line_number_digit_width(&self) -> usize {
        self.line_count().to_string().len().max(2)
    }

    pub(crate) fn line_number_gutter_width(&self) -> usize {
        self.line_number_digit_width() + 1
    }

    pub(crate) fn line_numbers_text(&self) -> String {
        let width = self.line_number_digit_width();
        (1..=self.line_count())
            .map(|line| format!("{line:>width$} "))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| String::from("No file"))
    }

    pub(crate) fn set_viewport_size(&mut self, height: usize, width: usize) {
        self.viewport_height = height.max(1);
        self.viewport_width = width.max(1);
        self.clamp_scrolls();
    }

    pub(crate) fn handle_standard_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if editable_char_modifiers(key.modifiers) => self.insert_char(c),
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_char(),
            KeyCode::Tab => self.insert_char('\t'),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Home => self.move_line_start(),
            KeyCode::End => self.move_line_end(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            _ => {}
        }
    }

    pub(crate) fn insert_char(&mut self, c: char) {
        self.record_undo();
        let row = self.cursor.row;
        let col = self.cursor.col;
        let byte = char_to_byte_index(&self.lines[row], col);
        self.lines[row].insert(byte, c);
        self.cursor.col += 1;
        self.mark_dirty();
    }

    pub(crate) fn insert_newline(&mut self) {
        self.record_undo();
        let row = self.cursor.row;
        let byte = char_to_byte_index(&self.lines[row], self.cursor.col);
        let rest = self.lines[row].split_off(byte);
        self.lines.insert(row + 1, rest);
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.mark_dirty();
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor.col == 0 && self.cursor.row == 0 {
            return;
        }
        self.record_undo();
        if self.cursor.col > 0 {
            let row = self.cursor.row;
            let end = char_to_byte_index(&self.lines[row], self.cursor.col);
            let start = char_to_byte_index(&self.lines[row], self.cursor.col - 1);
            self.lines[row].replace_range(start..end, "");
            self.cursor.col -= 1;
        } else {
            let row = self.cursor.row;
            let removed = self.lines.remove(row);
            self.cursor.row -= 1;
            self.cursor.col = char_len(&self.lines[self.cursor.row]);
            self.lines[self.cursor.row].push_str(&removed);
        }
        self.mark_dirty();
    }

    pub(crate) fn delete_char(&mut self) {
        let row = self.cursor.row;
        let line_len = char_len(&self.lines[row]);
        if self.cursor.col >= line_len {
            if row + 1 >= self.lines.len() {
                return;
            }
            self.record_undo();
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
            self.mark_dirty();
            return;
        }

        self.record_undo();
        let start = char_to_byte_index(&self.lines[row], self.cursor.col);
        let end = char_to_byte_index(&self.lines[row], self.cursor.col + 1);
        self.lines[row].replace_range(start..end, "");
        self.mark_dirty();
    }

    pub(crate) fn open_line_below(&mut self) {
        self.record_undo();
        let row = self.cursor.row + 1;
        self.lines.insert(row, String::new());
        self.cursor = Cursor { row, col: 0 };
        self.mark_dirty();
    }

    pub(crate) fn open_line_above(&mut self) {
        self.record_undo();
        let row = self.cursor.row;
        self.lines.insert(row, String::new());
        self.cursor = Cursor { row, col: 0 };
        self.mark_dirty();
    }

    pub(crate) fn delete_current_line(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        self.record_undo();
        let removed = self.lines.remove(self.cursor.row);
        self.yank = vec![removed];
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.row = self.cursor.row.min(self.lines.len() - 1);
        self.cursor.col = 0;
        self.mark_dirty();
    }

    pub(crate) fn yank_current_line(&mut self) {
        if let Some(line) = self.lines.get(self.cursor.row) {
            self.yank = vec![line.clone()];
        }
    }

    pub(crate) fn paste_after(&mut self) {
        if self.yank.is_empty() {
            return;
        }
        self.record_undo();
        let mut insert_at = self.cursor.row + 1;
        for line in &self.yank {
            self.lines.insert(insert_at, line.clone());
            insert_at += 1;
        }
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.mark_dirty();
    }

    pub(crate) fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.lines = snapshot.lines;
        self.cursor = snapshot.cursor;
        self.dirty = true;
        self.ensure_cursor_in_bounds();
    }

    pub(crate) fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = char_len(&self.lines[self.cursor.row]);
        }
        self.ensure_cursor_visible();
    }

    pub(crate) fn move_right(&mut self) {
        let line_len = char_len(&self.lines[self.cursor.row]);
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        } else if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
        self.ensure_cursor_visible();
    }

    pub(crate) fn move_up(&mut self) {
        self.cursor.row = self.cursor.row.saturating_sub(1);
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    pub(crate) fn move_down(&mut self) {
        if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
        }
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    pub(crate) fn move_line_start(&mut self) {
        self.cursor.col = 0;
        self.ensure_cursor_visible();
    }

    pub(crate) fn move_line_end(&mut self) {
        self.cursor.col = char_len(&self.lines[self.cursor.row]);
        self.ensure_cursor_visible();
    }

    pub(crate) fn set_cursor_from_view_position(&mut self, row: usize, col: usize) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.row = (self.scroll + row).min(self.lines.len() - 1);
        self.cursor.col =
            (self.horizontal_scroll + col).min(char_len(&self.lines[self.cursor.row]));
        self.ensure_cursor_visible();
    }

    pub(crate) fn scroll_vertical(&mut self, down: bool, amount: usize) {
        if down {
            self.scroll = self
                .scroll
                .saturating_add(amount)
                .min(self.max_vertical_scroll());
        } else {
            self.scroll = self.scroll.saturating_sub(amount);
        }
    }

    pub(crate) fn scroll_horizontal(&mut self, right: bool, amount: usize) {
        if right {
            self.horizontal_scroll = self
                .horizontal_scroll
                .saturating_add(amount)
                .min(self.max_horizontal_scroll());
        } else {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
        }
    }

    pub(crate) fn page_up(&mut self) {
        self.cursor.row = self.cursor.row.saturating_sub(PAGE_SIZE);
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    pub(crate) fn page_down(&mut self) {
        self.cursor.row = (self.cursor.row + PAGE_SIZE).min(self.lines.len() - 1);
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    pub(crate) fn record_undo(&mut self) {
        if self.undo_stack.len() == UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(EditorSnapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.pending_vim = None;
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    pub(crate) fn ensure_cursor_in_bounds(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.row = self.cursor.row.min(self.lines.len() - 1);
        self.cursor.col = self.cursor.col.min(char_len(&self.lines[self.cursor.row]));
    }

    pub(crate) fn ensure_cursor_visible(&mut self) {
        if self.cursor.row < self.scroll {
            self.scroll = self.cursor.row;
        } else if self.cursor.row >= self.scroll + self.viewport_height {
            self.scroll = self.cursor.row + 1 - self.viewport_height;
        }
        if self.cursor.col < self.horizontal_scroll {
            self.horizontal_scroll = self.cursor.col;
        } else if self.cursor.col >= self.horizontal_scroll + self.viewport_width {
            self.horizontal_scroll = self.cursor.col + 1 - self.viewport_width;
        }
        self.clamp_scrolls();
    }

    pub(crate) fn clamp_scrolls(&mut self) {
        self.scroll = self.scroll.min(self.max_vertical_scroll());
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    pub(crate) fn max_vertical_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.viewport_height)
    }

    pub(crate) fn max_horizontal_scroll(&self) -> usize {
        self.lines
            .iter()
            .map(|line| char_len(line))
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }
}
