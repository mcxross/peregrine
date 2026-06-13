use crate::workbench::prelude::*;

use crate::chat;
use crate::keybinds;
use crate::navigation::{self, NavigationCommand, NavigationIntent};
use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use crate::theme::ThemePalette;

impl App {
    pub(crate) fn render_explorer(&self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let items = self
            .explorer
            .visible_entries()
            .iter()
            .map(|entry| {
                let marker = if entry.is_dir {
                    if entry.expanded { "[-]" } else { "[+]" }
                } else {
                    "   "
                };
                let suffix = if entry.is_dir { "/" } else { "" };
                let label = format!(
                    "{}{} {}{}",
                    "  ".repeat(entry.depth),
                    marker,
                    entry.name,
                    suffix
                );
                let color = if entry.is_dir {
                    palette.accent
                } else {
                    palette.fg
                };
                ListItem::new(label).style(self.style_fg(color))
            })
            .collect::<Vec<_>>();
        let block = self.panel_block("Explorer", self.focus == FocusPane::Explorer);
        let mut state = ListState::default().with_selected(Some(self.explorer.selected()));
        let list = List::new(items)
            .block(block)
            .style(self.base_style())
            .highlight_style(self.selection_style())
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut state);
    }
}
