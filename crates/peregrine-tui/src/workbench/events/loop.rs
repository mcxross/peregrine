use crate::workbench::prelude::*;

use ratatui::crossterm::event::{
    self, Event, KeyEventKind,
};
use ratatui::DefaultTerminal;
use std::io;

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
