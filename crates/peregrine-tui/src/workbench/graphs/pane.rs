use super::super::{PAGE_SIZE, GraphTab, char_len};
use crate::sui::project::CliContext;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug)]
pub(crate) struct GraphPanes {
    pub(crate) active_tab: GraphTab,
    pub(crate) cfg: GraphPane,
    pub(crate) call_graph: GraphPane,
    pub(crate) type_graph: GraphPane,
}

impl Default for GraphPanes {
    fn default() -> Self {
        Self {
            active_tab: GraphTab::Cfg,
            cfg: GraphPane::default(),
            call_graph: GraphPane::default(),
            type_graph: GraphPane::default(),
        }
    }
}

impl GraphPanes {
    pub(crate) fn invalidate(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn get(&self, tab: GraphTab) -> Option<&GraphPane> {
        match tab {
            GraphTab::Cfg => Some(&self.cfg),
            GraphTab::CallGraph => Some(&self.call_graph),
            GraphTab::TypeGraph => Some(&self.type_graph),
        }
    }

    pub(crate) fn get_mut(&mut self, tab: GraphTab) -> Option<&mut GraphPane> {
        match tab {
            GraphTab::Cfg => Some(&mut self.cfg),
            GraphTab::CallGraph => Some(&mut self.call_graph),
            GraphTab::TypeGraph => Some(&mut self.type_graph),
        }
    }

    pub(crate) fn set_ready(&mut self, tab: GraphTab, document: GraphDocument) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Ready(document);
        }
    }

    pub(crate) fn set_message(&mut self, tab: GraphTab, message: String) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Message(message);
        }
    }

    pub(crate) fn set_loading(&mut self, tab: GraphTab) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Loading;
        }
    }
}

#[derive(Debug, Default)]
pub(crate) enum GraphPane {
    #[default]
    Empty,
    Loading,
    Ready(GraphDocument),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphDocument {
    pub(crate) title: String,
    pub(crate) text: String,
    pub(crate) line_count: usize,
    pub(crate) max_width: usize,
    pub(crate) scroll: usize,
    pub(crate) horizontal_scroll: usize,
    pub(crate) viewport_height: usize,
    pub(crate) viewport_width: usize,
}

impl GraphDocument {
    pub(crate) fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let line_count = text.lines().count().max(1);
        let max_width = text.lines().map(char_len).max().unwrap_or_default();

        Self {
            title: title.into(),
            text,
            line_count,
            max_width,
            scroll: 0,
            horizontal_scroll: 0,
            viewport_height: 1,
            viewport_width: 1,
        }
    }

    pub(crate) fn set_viewport_size(&mut self, height: usize, width: usize) {
        self.viewport_height = height.max(1);
        self.viewport_width = width.max(1);
        self.clamp_scrolls();
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll_vertical(false, 1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_vertical(true, 1),
            KeyCode::PageUp => self.scroll_vertical(false, PAGE_SIZE),
            KeyCode::PageDown => self.scroll_vertical(true, PAGE_SIZE),
            KeyCode::Left | KeyCode::Char('h') => self.scroll_horizontal(false, 1),
            KeyCode::Right | KeyCode::Char('l') => self.scroll_horizontal(true, 1),
            KeyCode::Home => {
                self.scroll = 0;
                self.horizontal_scroll = 0;
            }
            KeyCode::End => {
                self.scroll = self.max_vertical_scroll();
                self.horizontal_scroll = self.max_horizontal_scroll();
            }
            _ => {}
        }
    }

    pub(crate) fn scroll_vertical(&mut self, down: bool, amount: usize) {
        if down {
            self.scroll = self
                .scroll
                .saturating_add(amount)
                .min(self.max_vertical_scroll());
        } else {
            self.scroll = self.scroll.saturating_sub(amount);
        }
    }

    pub(crate) fn scroll_horizontal(&mut self, right: bool, amount: usize) {
        if right {
            self.horizontal_scroll = self
                .horizontal_scroll
                .saturating_add(amount)
                .min(self.max_horizontal_scroll());
        } else {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
        }
    }

    pub(crate) fn clamp_scrolls(&mut self) {
        self.scroll = self.scroll.min(self.max_vertical_scroll());
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    pub(crate) fn max_vertical_scroll(&self) -> usize {
        self.line_count.saturating_sub(self.viewport_height)
    }

    pub(crate) fn max_horizontal_scroll(&self) -> usize {
        self.max_width
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchGraphContext {
    pub(crate) context: CliContext,
    pub(crate) module_filters: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct GraphLoadResult {
    pub(crate) tab: GraphTab,
    pub(crate) result: Result<GraphDocument, String>,
}
