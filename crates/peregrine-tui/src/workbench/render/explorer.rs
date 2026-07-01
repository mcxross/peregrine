use crate::workbench::prelude::*;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem, ListState};

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
