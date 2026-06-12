use super::super::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

impl App {
    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        frame.buffer_mut().set_style(area, self.base_style());
        if !self.startup.is_workbench() {
            self.render_startup(frame, area);
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(6), Constraint::Length(1)])
            .split(area);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
            .split(rows[0]);

        self.layout.explorer = columns[0];
        self.layout.bottom_bar = rows[1];
        self.render_explorer(frame, columns[0]);
        self.render_center(frame, columns[1]);
        self.render_bottom_bar(frame, rows[1]);
    }
}
