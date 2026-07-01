use crate::workbench::GraphTab;
use crate::workbench::prelude::*;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

impl App {
    pub(crate) fn render_graph(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let focused = self.focus == FocusPane::Editor;

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(3)])
            .split(area);

        self.layout.graph_tabs = rows[0];

        let graph_tab_labels = ["cfg", "call graph", "type graph"];

        let mut spans = Vec::new();
        let mut hit_areas = Vec::new();
        let mut current_x = rows[0].x;

        for (i, &label) in graph_tab_labels.iter().enumerate() {
            let is_active = i == self.graphs.active_tab.index();

            if i > 0 {
                let sep = " │ ";
                spans.push(Span::raw(sep).style(self.muted_style()));
                current_x += sep.chars().count() as u16;
            }

            let style = if is_active {
                self.style_fg(palette.accent).add_modifier(Modifier::BOLD)
            } else {
                self.muted_style()
            };

            let text = if is_active {
                format!("▸ {}", label)
            } else {
                format!("  {}", label)
            };

            let width = text.chars().count() as u16;
            hit_areas.push(Rect {
                x: current_x,
                y: rows[0].y,
                width,
                height: 1,
            });

            spans.push(Span::styled(text, style));
            current_x += width;
        }

        self.layout.graph_tab_hit_areas = hit_areas
            .into_iter()
            .zip(GraphTab::ALL)
            .map(|(area, tab)| (tab, area))
            .collect();

        let tabs_paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(tabs_paragraph, rows[0]);

        let tab = self.graphs.active_tab;
        let content_area = rows[1];

        let message_style = self.muted_style();
        let graph_style = self.style_fg(palette.syntax.text);
        let block = self.panel_block("", focused);
        let inner = inner_rect(content_area);

        match self.graphs.get_mut(tab) {
            Some(GraphPane::Ready(document)) => {
                document.set_viewport_size(inner.height as usize, inner.width as usize);
                let paragraph = Paragraph::new(document.text.as_str())
                    .style(graph_style)
                    .block(block)
                    .scroll((
                        usize_to_u16_saturating(document.scroll),
                        usize_to_u16_saturating(document.horizontal_scroll),
                    ));
                frame.render_widget(paragraph, content_area);
            }
            Some(GraphPane::Message(message)) => {
                let paragraph = Paragraph::new(message.as_str())
                    .style(message_style)
                    .block(block);
                frame.render_widget(paragraph, content_area);
            }
            Some(GraphPane::Loading) => {
                let paragraph = Paragraph::new(format!("Loading {}...", tab.title()))
                    .style(message_style)
                    .block(block);
                frame.render_widget(paragraph, content_area);
            }
            Some(GraphPane::Empty) | None => {
                let paragraph = Paragraph::new(format!(
                    "{} is not loaded. Press Enter to load.",
                    tab.title()
                ))
                .style(message_style)
                .block(block);
                frame.render_widget(paragraph, content_area);
            }
        }
    }

    pub(crate) fn render_bytecode(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.theme.palette();
        let focused = self.focus == FocusPane::Editor;
        let light_theme = self.theme.current().is_light();
        let message_style = self.muted_style();
        let message_block = self.panel_block("Bytecode", focused);

        match &mut self.bytecode {
            BytecodePane::Ready(session) => {
                session.render(frame, area, palette, focused, light_theme);
            }
            BytecodePane::Selecting(selector) => {
                selector.render(frame, area, palette, focused);
            }
            BytecodePane::Loading(load) => {
                let paragraph = Paragraph::new(format!(
                    "Loading bytecode for {}::{}...",
                    load.package_name, load.module_name
                ))
                .style(message_style)
                .block(message_block);
                frame.render_widget(paragraph, area);
            }
            BytecodePane::Message(message) => {
                let paragraph = Paragraph::new(message.as_str())
                    .style(message_style)
                    .block(message_block);
                frame.render_widget(paragraph, area);
            }
            BytecodePane::Empty => {
                let paragraph =
                    Paragraph::new("Bytecode is not loaded. Press Enter to resolve modules.")
                        .style(message_style)
                        .block(message_block);
                frame.render_widget(paragraph, area);
            }
        }
    }
}
