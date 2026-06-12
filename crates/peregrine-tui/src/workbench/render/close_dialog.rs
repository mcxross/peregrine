use crate::workbench::prelude::*;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph, Wrap};

impl App {
    pub(crate) fn render_close_confirmation(&mut self, frame: &mut Frame<'_>) {
        let Some(confirmation) = self.pending_close.as_ref() else {
            self.layout.close_dialog_hit_areas.clear();
            return;
        };
        let selected = confirmation.selected;
        let error = confirmation.error.clone();
        let label = self
            .editor
            .document_label(confirmation.document_id)
            .unwrap_or("file")
            .to_string();
        let area = centered_rect(frame.area(), 58, 9);
        frame.render_widget(Clear, area);
        frame.render_widget(self.panel_block("Unsaved Changes", true), area);
        let inner = inner_rect(area);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);
        let body = vec![
            Line::from("Save changes before closing this file?"),
            Line::from(label),
            Line::from(error.unwrap_or_default()).style(self.style_fg(self.palette().warning)),
        ];
        frame.render_widget(
            Paragraph::new(body)
                .style(self.base_style())
                .wrap(Wrap { trim: false }),
            rows[0],
        );

        let option_area = Rect::new(
            rows[1].x.saturating_add(1),
            rows[1].y,
            rows[1].width.saturating_sub(2),
            1,
        );
        let options = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(option_area);
        self.layout.close_dialog_hit_areas = CloseChoice::ALL
            .into_iter()
            .zip(options.iter().copied())
            .collect();
        for (choice, option) in CloseChoice::ALL.into_iter().zip(options.iter().copied()) {
            let style = if choice == selected {
                self.selection_style().add_modifier(Modifier::UNDERLINED)
            } else {
                self.muted_style()
            };
            frame.render_widget(
                Paragraph::new(format!("[{}] {}", choice.shortcut(), choice.label())).style(style),
                option,
            );
        }
        frame.render_widget(
            Paragraph::new("Enter confirms · Esc cancels").style(self.muted_style()),
            rows[2],
        );
    }
}
