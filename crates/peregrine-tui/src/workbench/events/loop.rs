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

impl App {
    pub(crate) fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<WorkbenchExit> {
        self.mode = AppMode::Workbench;
        self.exit = None;
        loop {
            self.drain_startup_task();
            self.drain_bytecode_loader();
            self.drain_graph_loader();
            self.tick_chat();
            self.refresh_shared_theme();
            terminal.draw(|frame| self.render(frame))?;
            if let Some(exit) = self.exit {
                return Ok(exit);
            }
            if !event::poll(self.redraw_interval())? {
                continue;
            }
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key);
                    }
                }
                Event::Mouse(mouse) => self.handle_mouse_event(mouse),
                Event::Paste(pasted) if self.active_tab == WorkbenchTab::Chat => {
                    let action = self.chat.handle_paste(&self.explorer.root, pasted);
                    self.apply_chat_action(action);
                }
                _ => {}
            }
        }
    }
}
