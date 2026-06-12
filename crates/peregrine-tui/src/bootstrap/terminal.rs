use crate::workbench::{App, WorkbenchExit};
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use std::io;

pub fn run_tui(app: &mut App) -> io::Result<WorkbenchExit> {
    let mut terminal = ratatui::try_init()?;
    let mut terminal_guard = WorkbenchTerminalGuard::new();
    if let Err(error) = execute!(io::stdout(), EnableMouseCapture) {
        let _ = terminal_guard.restore();
        return Err(error);
    }
    terminal_guard.mouse_capture_enabled = true;

    let result = app.run(&mut terminal);
    let cleanup_result = terminal_guard.restore();

    match (result, cleanup_result) {
        (Ok(exit), Ok(())) => Ok(exit),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

struct WorkbenchTerminalGuard {
    active: bool,
    mouse_capture_enabled: bool,
}

impl WorkbenchTerminalGuard {
    fn new() -> Self {
        Self {
            active: true,
            mouse_capture_enabled: false,
        }
    }

    fn restore(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }

        let mouse_result = if self.mouse_capture_enabled {
            execute!(io::stdout(), DisableMouseCapture)
        } else {
            Ok(())
        };
        ratatui::restore();
        self.active = false;
        mouse_result
    }
}

impl Drop for WorkbenchTerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
