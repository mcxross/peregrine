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
use std::time::{Duration, Instant};

use super::super::{format_elapsed, package_load_spinner, package_load_status_spans};

impl App {
    pub(crate) fn render_bottom_bar(&self, frame: &mut Frame<'_>, area: Rect) {
        let mut spans = Vec::new();
        if let Some(state) = self.package_load_running_state() {
            spans.push(Span::styled(
                package_load_spinner(state.started_at),
                self.style_fg(self.palette().accent)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" package: "));
            spans.push(Span::styled(
                "running",
                self.style_fg(self.palette().warning),
            ));
            spans.push(Span::styled(
                format!(" {}", format_elapsed(state.started_at.elapsed())),
                self.muted_style(),
            ));
        } else if let Some(report) = &self.package_load_report {
            spans.extend(package_load_status_spans(report, self));
        } else {
            spans.push(Span::styled("package: ", self.muted_style()));
            spans.push(Span::styled("no status yet", self.muted_style()));
        }

        if !self.status.is_empty() {
            spans.push(Span::styled(" | ", self.muted_style()));
            spans.push(Span::styled(self.status.clone(), self.muted_style()));
        }

        if self.active_tab == WorkbenchTab::Chat {
            spans.push(Span::styled(" | ", self.muted_style()));
            spans.push(Span::styled(
                self.chat.status().to_string(),
                self.muted_style(),
            ));
        }

        let paragraph = Paragraph::new(Line::from(spans)).style(self.base_style());
        frame.render_widget(paragraph, area);
    }

    pub(crate) fn package_load_running_state(&self) -> Option<&PackageLoadRunningState> {
        match &self.startup {
            WorkbenchStartupState::PackageLoadRunning(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn redraw_interval(&self) -> Duration {
        if self.active_tab == WorkbenchTab::Chat {
            Duration::from_millis(16)
        } else if self.package_load_running_state().is_some()
            || self.bytecode_loader_rx.is_some()
            || self.graph_loader_rx.is_some()
        {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(250)
        }
    }
}
