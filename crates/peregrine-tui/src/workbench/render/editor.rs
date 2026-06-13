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

use crate::theme::ThemeName;
use crate::workbench_render::{
    RenderedWorkbenchDocument, is_markdown_path, render_workbench_document,
};

impl App {
    pub(crate) fn editor_text_area(&self) -> Rect {
        self.editor_areas_with_gutter(self.layout.editor, !self.markdown_preview_enabled())
            .1
    }

    pub(crate) fn markdown_preview_enabled(&self) -> bool {
        self.editor.path.as_deref().is_some_and(is_markdown_path) && !self.editor_source_editing()
    }

    pub(crate) fn editor_source_editing(&self) -> bool {
        match self.editor_mode {
            EditorMode::Standard => self.standard_editor_editing,
            EditorMode::Vim => self.vim_state == VimState::Insert,
        }
    }

    pub(crate) fn rendered_editor_document(
        &mut self,
        source: &str,
        width: usize,
        markdown_preview: bool,
    ) -> RenderedWorkbenchDocument {
        let path = self.editor.path.clone();
        let root = self.explorer.root.clone();
        let theme = self.theme.current_name();

        if let Some(cache) = &self.editor_render_cache
            && cache.path == path
            && cache.source == source
            && cache.theme == theme
            && cache.markdown_preview == markdown_preview
            && cache.width == width
            && cache.root == root
        {
            return cache.document.clone();
        }

        let document = render_workbench_document(
            source,
            path.as_deref(),
            self.palette(),
            markdown_preview,
            width,
            Some(&root),
        );
        self.editor_render_cache = Some(EditorRenderCache {
            path,
            source: source.to_string(),
            theme,
            markdown_preview,
            width,
            root,
            document: document.clone(),
        });
        document
    }

    pub(crate) fn editor_areas_with_gutter(&self, area: Rect, show_gutter: bool) -> (Rect, Rect) {
        let inner = inner_rect(area);
        if inner.width == 0 {
            return (inner, inner);
        }

        let desired_gutter_width = usize_to_u16_saturating(self.editor.line_number_gutter_width());
        let gutter_width = if !show_gutter || inner.width <= 1 {
            0
        } else {
            desired_gutter_width.min(inner.width.saturating_sub(1))
        };
        let gutter = Rect {
            x: inner.x,
            y: inner.y,
            width: gutter_width,
            height: inner.height,
        };
        let text = Rect {
            x: inner.x.saturating_add(gutter_width),
            y: inner.y,
            width: inner.width.saturating_sub(gutter_width),
            height: inner.height,
        };

        (gutter, text)
    }

    pub(crate) fn render_editor(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let markdown_preview = self.markdown_preview_enabled();
        let (gutter_area, text_area) = self.editor_areas_with_gutter(area, !markdown_preview);
        let inner_height = text_area.height as usize;
        let inner_width = text_area.width as usize;
        self.editor.set_viewport_size(inner_height, inner_width);
        let title = format!(
            "{}{} [{}]",
            self.editor.display_name(),
            if self.editor.dirty { " *" } else { "" },
            self.editor_mode_label()
        );
        let block = self.panel_block(title, self.focus == FocusPane::Editor);
        frame.render_widget(block, area);

        let text = self.editor.text();
        let rendered = self.rendered_editor_document(&text, inner_width, markdown_preview);

        if rendered.show_gutter && gutter_area.width > 0 && gutter_area.height > 0 {
            let numbers = self.editor.line_numbers_text();
            let gutter = Paragraph::new(numbers)
                .style(self.muted_style())
                .scroll((usize_to_u16_saturating(self.editor.scroll), 0));
            frame.render_widget(gutter, gutter_area);
        }

        if !rendered.show_cursor {
            self.editor.scroll = self
                .editor
                .scroll
                .min(rendered.lines.len().saturating_sub(inner_height));
        }

        let show_cursor = rendered.show_cursor;
        let paragraph = Paragraph::new(rendered.lines)
            .style(self.style_fg(self.palette().syntax.text))
            .scroll((
                usize_to_u16_saturating(self.editor.scroll),
                usize_to_u16_saturating(self.editor.horizontal_scroll),
            ));
        frame.render_widget(paragraph, text_area);

        if show_cursor && self.focus == FocusPane::Editor && self.active_tab == WorkbenchTab::Code {
            let row = self.editor.cursor.row.saturating_sub(self.editor.scroll);
            let col = self
                .editor
                .cursor
                .col
                .saturating_sub(self.editor.horizontal_scroll);
            if self.editor.cursor.row >= self.editor.scroll
                && self.editor.cursor.col >= self.editor.horizontal_scroll
                && row < inner_height
                && col < inner_width
            {
                let x = usize_to_u16_saturating(col);
                let y = usize_to_u16_saturating(row);
                frame.set_cursor_position(Position::new(text_area.x + x, text_area.y + y));
            }
        }
    }
}
