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

use crate::tabs::{TabNav, tab_hit_areas};

impl App {
    pub(crate) fn render_center(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        self.layout.tabs = rows[0];
        self.layout.tab_hit_areas = tab_hit_areas(&WORKBENCH_TAB_LABELS, rows[0])
            .into_iter()
            .zip(WorkbenchTab::ALL)
            .map(|(area, tab)| (tab, area))
            .collect();

        let tabs = TabNav::new(&WORKBENCH_TAB_LABELS, self.active_tab.index())
            .style(self.muted_style())
            .highlight_style(self.style_fg(palette.accent).add_modifier(Modifier::BOLD))
            .border_style(self.border_style(self.focus == FocusPane::Tabs))
            .highlight_bold(true);
        frame.render_widget(tabs, rows[0]);

        match self.active_tab {
            WorkbenchTab::Code => {
                let code_rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(2)])
                    .split(rows[1]);
                self.layout.file_tabs = code_rows[0];
                self.layout.editor = code_rows[1];
                self.render_file_tabs(frame, code_rows[0]);
                self.render_editor(frame, code_rows[1]);
            }
            WorkbenchTab::Bytecode => self.render_bytecode(frame, rows[1]),
            WorkbenchTab::Cfg | WorkbenchTab::CallGraph | WorkbenchTab::TypeGraph => {
                self.render_graph(frame, rows[1], self.active_tab)
            }
            WorkbenchTab::Chat => {
                self.chat
                    .render(frame, rows[1], self.focus == FocusPane::Input, palette);
            }
        }
        if self.active_tab != WorkbenchTab::Code {
            self.layout.file_tabs = Rect::default();
            self.layout.file_tab_hit_areas.clear();
            self.layout.editor = rows[1];
        }
    }
}
