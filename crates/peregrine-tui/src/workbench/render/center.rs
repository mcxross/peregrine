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

use crate::tabs::{tab_hit_areas, TabNav};

impl App {
    pub(crate) fn render_center(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        self.layout.tabs = rows[0];
        self.layout.editor = rows[1];
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
            WorkbenchTab::Code => self.render_editor(frame, rows[1]),
            WorkbenchTab::Bytecode => self.render_bytecode(frame, rows[1]),
            WorkbenchTab::Cfg | WorkbenchTab::CallGraph | WorkbenchTab::TypeGraph => {
                self.render_graph(frame, rows[1], self.active_tab)
            }
            WorkbenchTab::Chat => {
                self.chat
                    .render(frame, rows[1], self.focus == FocusPane::Input);
            }
        }

    }

}
