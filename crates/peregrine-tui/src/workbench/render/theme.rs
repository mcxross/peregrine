use crate::workbench::prelude::*;

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

use crate::theme::ThemePalette;

impl App {
    pub(crate) fn palette(&self) -> ThemePalette {
        self.theme.palette()
    }

    pub(crate) fn style_fg(&self, color: Color) -> Style {
        Style::default().fg(color).bg(self.palette().bg)
    }

    pub(crate) fn base_style(&self) -> Style {
        let palette = self.palette();
        Style::default().fg(palette.fg).bg(palette.bg)
    }

    pub(crate) fn muted_style(&self) -> Style {
        self.style_fg(self.palette().muted)
    }

    pub(crate) fn border_style(&self, focused: bool) -> Style {
        let palette = self.palette();
        self.style_fg(if focused {
            palette.accent
        } else {
            palette.graph.edge
        })
    }

    pub(crate) fn title_style(&self, focused: bool) -> Style {
        let palette = self.palette();
        self.style_fg(if focused { palette.accent } else { palette.fg })
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            })
    }

    pub(crate) fn selection_style(&self) -> Style {
        let palette = self.palette();
        Style::default()
            .fg(palette.fg)
            .bg(palette.selection)
            .add_modifier(Modifier::BOLD)
    }

    pub(crate) fn panel_block(&self, title: impl Into<String>, focused: bool) -> Block<'static> {
        let title = focused_title(&title.into(), focused);
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(self.base_style())
            .border_style(self.border_style(focused))
            .title_style(self.title_style(focused))
    }
}
