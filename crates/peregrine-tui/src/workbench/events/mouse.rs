use crate::workbench::prelude::*;

use ratatui::crossterm::event::{
    MouseButton, MouseEvent,
    MouseEventKind,
};

impl App {
    pub(crate) fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if self.pending_close.is_some() {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
                && let Some(choice) =
                    self.layout
                        .close_dialog_hit_areas
                        .iter()
                        .find_map(|(choice, area)| {
                            rect_contains(*area, mouse.column, mouse.row).then_some(*choice)
                        })
            {
                self.resolve_close_choice(choice);
            }
            return;
        }
        if !self.startup.is_workbench() {
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_left_click(mouse.column, mouse.row)
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Up)
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Down)
            }
            MouseEventKind::ScrollLeft => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Left)
            }
            MouseEventKind::ScrollRight => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Right)
            }
            _ => {}
        }
    }

    pub(crate) fn handle_scroll(&mut self, x: u16, y: u16, direction: ScrollDirection) {
        if rect_contains(self.layout.file_tabs, x, y) {
            self.set_focus(FocusPane::FileTabs);
            match direction {
                ScrollDirection::Up | ScrollDirection::Left => {
                    self.editor.page_left(self.layout.file_tabs.width)
                }
                ScrollDirection::Down | ScrollDirection::Right => {
                    self.editor.page_right(self.layout.file_tabs.width)
                }
            }
            return;
        }

        if rect_contains(self.layout.explorer, x, y) {
            self.set_focus(FocusPane::Explorer);
            self.scroll_explorer(direction);
            return;
        }

        if rect_contains(self.layout.editor, x, y) {
            match self.active_tab {
                WorkbenchTab::Editor => {
                    self.set_focus(FocusPane::Editor);
                    self.scroll_editor(direction);
                }
                WorkbenchTab::Bytecode => {
                    self.set_focus(FocusPane::Editor);
                    self.scroll_bytecode(direction);
                }
                WorkbenchTab::Graphs => {
                    self.set_focus(FocusPane::Editor);
                    self.scroll_graph(direction);
                }
                WorkbenchTab::Chat => {
                    self.set_focus(FocusPane::Input);
                    self.chat.scroll(direction, MOUSE_VERTICAL_SCROLL_STEP);
                }
            }
            return;
        }
    }

    pub(crate) fn scroll_explorer(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Up => {
                for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                    self.explorer.select_previous();
                }
            }
            ScrollDirection::Down => {
                for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                    self.explorer.select_next();
                }
            }
            ScrollDirection::Left | ScrollDirection::Right => {}
        }
    }

    pub(crate) fn scroll_editor(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Up => self
                .editor
                .scroll_vertical(false, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Down => self
                .editor
                .scroll_vertical(true, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Left => self
                .editor
                .scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP),
            ScrollDirection::Right => self
                .editor
                .scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP),
        }
    }

    pub(crate) fn scroll_bytecode(&mut self, direction: ScrollDirection) {
        match &mut self.bytecode {
            BytecodePane::Selecting(selector) => match direction {
                ScrollDirection::Up => {
                    for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                        selector.select_previous();
                    }
                }
                ScrollDirection::Down => {
                    for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                        selector.select_next();
                    }
                }
                ScrollDirection::Left | ScrollDirection::Right => {}
            },
            BytecodePane::Ready(session) => match direction {
                ScrollDirection::Up => {
                    session.scroll_vertical(false, MOUSE_VERTICAL_SCROLL_STEP as u16)
                }
                ScrollDirection::Down => {
                    session.scroll_vertical(true, MOUSE_VERTICAL_SCROLL_STEP as u16)
                }
                ScrollDirection::Left => {
                    session.scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP as u16)
                }
                ScrollDirection::Right => {
                    session.scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP as u16)
                }
            },
            BytecodePane::Empty | BytecodePane::Loading(_) | BytecodePane::Message(_) => {}
        }
    }

    pub(crate) fn scroll_graph(&mut self, direction: ScrollDirection) {
        let Some(GraphPane::Ready(document)) = self.graphs.get_mut(self.graphs.active_tab) else {
            return;
        };

        match direction {
            ScrollDirection::Up => document.scroll_vertical(false, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Down => document.scroll_vertical(true, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Left => {
                document.scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP)
            }
            ScrollDirection::Right => {
                document.scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP)
            }
        }
    }

    pub(crate) fn handle_left_click(&mut self, x: u16, y: u16) {
        if let Some(target) = self.clicked_file_tab(x, y) {
            self.set_focus(FocusPane::FileTabs);
            match target {
                FileTabHitTarget::Previous => {
                    self.editor.page_left(self.layout.file_tabs.width);
                }
                FileTabHitTarget::Activate(id) => self.activate_document(id),
                FileTabHitTarget::Close(id) => self.request_close_document(id),
                FileTabHitTarget::Next => {
                    self.editor.page_right(self.layout.file_tabs.width);
                }
            }
            return;
        }

        if let Some(tab) = self.clicked_graph_tab(x, y) {
            self.graphs.active_tab = tab;
            self.set_focus(FocusPane::Editor);
            return;
        }

        if let Some(tab) = self.clicked_tab(x, y) {
            self.set_active_tab(tab);
            if tab == WorkbenchTab::Chat {
                self.set_focus(FocusPane::Input);
            } else {
                self.set_focus(FocusPane::Editor);
            }
            return;
        }

        if rect_contains(self.layout.tabs, x, y) {
            self.set_focus(FocusPane::Tabs);
            return;
        }

        if rect_contains(self.layout.explorer, x, y) {
            self.handle_explorer_click(x, y);
            return;
        }

        if rect_contains(self.layout.editor, x, y) {
            self.handle_editor_click(x, y);
            return;
        }
    }

    pub(crate) fn clicked_tab(&self, x: u16, y: u16) -> Option<WorkbenchTab> {
        self.layout
            .tab_hit_areas
            .iter()
            .find_map(|(tab, area)| rect_contains(*area, x, y).then_some(*tab))
    }

    pub(crate) fn clicked_graph_tab(&self, x: u16, y: u16) -> Option<crate::workbench::types::GraphTab> {
        self.layout
            .graph_tab_hit_areas
            .iter()
            .find_map(|(tab, area)| rect_contains(*area, x, y).then_some(*tab))
    }

    pub(crate) fn clicked_file_tab(&self, x: u16, y: u16) -> Option<FileTabHitTarget> {
        self.layout
            .file_tab_hit_areas
            .iter()
            .find_map(|hit| rect_contains(hit.area, x, y).then_some(hit.target))
    }

    pub(crate) fn handle_explorer_click(&mut self, x: u16, y: u16) {
        self.set_focus(FocusPane::Explorer);
        let inner = inner_rect(self.layout.explorer);
        if !rect_contains(inner, x, y) {
            return;
        }

        let row = usize::from(y.saturating_sub(inner.y));
        if row >= self.explorer.visible_entries().len() {
            return;
        }

        self.explorer.selected = row;
        match self.explorer.activate_selected() {
            ExplorerAction::OpenFile(path) => self.open_file(path),
            ExplorerAction::ToggledDirectory => {
                self.status = String::from("Directory tree updated");
            }
            ExplorerAction::None => {}
        }
    }

    pub(crate) fn handle_editor_click(&mut self, x: u16, y: u16) {
        if self.active_tab == WorkbenchTab::Chat {
            self.set_focus(FocusPane::Input);
            let action = self.chat.handle_left_click(self.layout.editor, x, y);
            self.apply_chat_action(action);
            return;
        }
        self.set_focus(FocusPane::Editor);
        if self.active_tab == WorkbenchTab::Editor {
            if self.markdown_preview_enabled() {
                return;
            }
            let text_area = self.editor_text_area();
            if rect_contains(text_area, x, y) {
                self.editor.set_cursor_from_view_position(
                    usize::from(y.saturating_sub(text_area.y)),
                    usize::from(x.saturating_sub(text_area.x)),
                );
            }
        }
    }
}
