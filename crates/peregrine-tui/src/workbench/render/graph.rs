use crate::workbench::prelude::*;

use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    pub(crate) fn render_graph(&mut self, frame: &mut Frame<'_>, area: Rect, tab: WorkbenchTab) {
        let focused = self.focus == FocusPane::Editor;
        let message_style = self.muted_style();
        let graph_style = self.style_fg(self.palette().syntax.text);
        let block = self.panel_block(tab.title(), focused);
        let inner = inner_rect(area);

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
                frame.render_widget(paragraph, area);
            }
            Some(GraphPane::Message(message)) => {
                let paragraph = Paragraph::new(message.as_str())
                    .style(message_style)
                    .block(block);
                frame.render_widget(paragraph, area);
            }
            Some(GraphPane::Loading) => {
                let paragraph = Paragraph::new(format!("Loading {}...", tab.title()))
                    .style(message_style)
                    .block(block);
                frame.render_widget(paragraph, area);
            }
            Some(GraphPane::Empty) | None => {
                let paragraph = Paragraph::new(format!(
                    "{} is not loaded. Press Enter to load.",
                    tab.title()
                ))
                .style(message_style)
                .block(block);
                frame.render_widget(paragraph, area);
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
