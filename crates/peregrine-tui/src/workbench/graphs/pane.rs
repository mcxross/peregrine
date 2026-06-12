use crate::sui::project::CliContext;
use super::super::{char_len, WorkbenchTab, PAGE_SIZE};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Default)]
pub(crate) struct GraphPanes {
    pub(crate) cfg: GraphPane,
    pub(crate) call_graph: GraphPane,
    pub(crate) type_graph: GraphPane,
}

impl GraphPanes {
    pub(crate) fn invalidate(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn get(&self, tab: WorkbenchTab) -> Option<&GraphPane> {
        match tab {
            WorkbenchTab::Cfg => Some(&self.cfg),
            WorkbenchTab::CallGraph => Some(&self.call_graph),
            WorkbenchTab::TypeGraph => Some(&self.type_graph),
            WorkbenchTab::Code | WorkbenchTab::Bytecode | WorkbenchTab::Chat => None,
        }
    }

    pub(crate) fn get_mut(&mut self, tab: WorkbenchTab) -> Option<&mut GraphPane> {
        match tab {
            WorkbenchTab::Cfg => Some(&mut self.cfg),
            WorkbenchTab::CallGraph => Some(&mut self.call_graph),
            WorkbenchTab::TypeGraph => Some(&mut self.type_graph),
            WorkbenchTab::Code | WorkbenchTab::Bytecode | WorkbenchTab::Chat => None,
        }
    }

    pub(crate) fn set_ready(&mut self, tab: WorkbenchTab, document: GraphDocument) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Ready(document);
        }
    }

    pub(crate) fn set_message(&mut self, tab: WorkbenchTab, message: String) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Message(message);
        }
    }

    pub(crate) fn set_loading(&mut self, tab: WorkbenchTab) {
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
    pub(crate) tab: WorkbenchTab,
    pub(crate) result: Result<GraphDocument, String>,
}
