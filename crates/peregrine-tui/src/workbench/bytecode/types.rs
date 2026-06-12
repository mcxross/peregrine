use super::session::BytecodeSession;
use crate::sui::project::{BytecodeTarget, CliContext};
use crate::theme::ThemePalette;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;
use std::ffi::OsStr;
use std::path::PathBuf;

use super::super::PAGE_SIZE;

#[derive(Debug)]
pub(crate) struct BytecodeLoadState {
    pub(crate) key: BytecodeTargetKey,
    pub(crate) package_name: String,
    pub(crate) module_name: String,
    pub(crate) stamp: BytecodeCacheStamp,
    pub(crate) epoch: u64,
}

#[derive(Debug)]
pub(crate) struct BytecodeLoadResult {
    pub(crate) epoch: u64,
    pub(crate) key: BytecodeTargetKey,
    pub(crate) stamp: BytecodeCacheStamp,
    pub(crate) result: Result<BytecodeSession, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BytecodeCacheEntry {
    pub(crate) stamp: BytecodeCacheStamp,
    pub(crate) session: BytecodeSession,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BytecodeCacheStamp {
    pub(crate) file_count: u64,
    pub(crate) total_len: u64,
    pub(crate) latest_modified_nanos: u128,
}

#[derive(Debug)]
pub(crate) struct BytecodeOptions {
    pub(crate) context: CliContext,
    pub(crate) key: BytecodeOptionsKey,
    pub(crate) package_name: String,
    pub(crate) targets: Vec<BytecodeTarget>,
}

impl BytecodeOptions {
    pub(crate) fn new(context: CliContext, file: Option<String>, mut targets: Vec<BytecodeTarget>) -> Self {
        targets.sort_by(|left, right| {
            left.file_path
                .cmp(&right.file_path)
                .then_with(|| left.module_name.cmp(&right.module_name))
        });
        let package_name = bytecode_package_name(&context);

        Self {
            key: BytecodeOptionsKey {
                package_root: context.package_root.clone(),
                file,
            },
            context,
            package_name,
            targets,
        }
    }

    pub(crate) fn contains_target_key(&self, key: &BytecodeTargetKey) -> bool {
        self.targets
            .iter()
            .any(|target| BytecodeTargetKey::new(&self.context, target) == *key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BytecodeOptionsKey {
    pub(crate) package_root: PathBuf,
    pub(crate) file: Option<String>,
}

#[derive(Debug)]
pub(crate) struct BytecodeSelector {
    pub(crate) context: CliContext,
    pub(crate) key: BytecodeOptionsKey,
    pub(crate) package_name: String,
    pub(crate) targets: Vec<BytecodeTarget>,
    pub(crate) selected: usize,
}

impl BytecodeSelector {
    pub(crate) fn new(options: BytecodeOptions) -> Self {
        Self {
            context: options.context,
            key: options.key,
            package_name: options.package_name,
            targets: options.targets,
            selected: 0,
        }
    }

    pub(crate) fn matches(&self, options: &BytecodeOptions) -> bool {
        self.key == options.key && self.targets == options.targets
    }

    pub(crate) fn selected_request(&self) -> Option<BytecodeRequest> {
        self.targets
            .get(self.selected)
            .cloned()
            .map(|target| BytecodeRequest::new(self.context.clone(), target))
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::PageUp => self.select_previous_page(),
            KeyCode::PageDown => self.select_next_page(),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => {
                self.selected = self.targets.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    pub(crate) fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub(crate) fn select_next(&mut self) {
        if self.selected + 1 < self.targets.len() {
            self.selected += 1;
        }
    }

    pub(crate) fn select_previous_page(&mut self) {
        self.selected = self.selected.saturating_sub(PAGE_SIZE);
    }

    pub(crate) fn select_next_page(&mut self) {
        self.selected = self
            .selected
            .saturating_add(PAGE_SIZE)
            .min(self.targets.len().saturating_sub(1));
    }

    pub(crate) fn render(&self, frame: &mut Frame<'_>, area: Rect, palette: ThemePalette, focused: bool) {
        let base_style = Style::default().fg(palette.fg).bg(palette.bg);
        let border_style = Style::default()
            .fg(if focused {
                palette.accent
            } else {
                palette.graph.edge
            })
            .bg(palette.bg);
        let title_style = Style::default()
            .fg(if focused { palette.accent } else { palette.fg })
            .bg(palette.bg)
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let items = self
            .targets
            .iter()
            .map(|target| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        target.module_name.clone(),
                        Style::default()
                            .fg(palette.accent)
                            .bg(palette.bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        target.file_path.clone(),
                        Style::default().fg(palette.muted).bg(palette.bg),
                    ),
                ]))
                .style(base_style)
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(self.selected));
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Select Bytecode Module - {}", self.package_name))
                    .style(base_style)
                    .border_style(border_style)
                    .title_style(title_style),
            )
            .style(base_style)
            .highlight_style(
                Style::default()
                    .fg(palette.fg)
                    .bg(palette.selection)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BytecodeTargetKey {
    pub(crate) package_root: PathBuf,
    pub(crate) module_name: String,
    pub(crate) source_path: PathBuf,
}

impl BytecodeTargetKey {
    pub(crate) fn new(context: &CliContext, target: &BytecodeTarget) -> Self {
        Self {
            package_root: context.package_root.clone(),
            module_name: target.module_name.clone(),
            source_path: target.source_path.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct BytecodeRequest {
    pub(crate) context: CliContext,
    pub(crate) key: BytecodeTargetKey,
    pub(crate) package_name: String,
}

impl BytecodeRequest {
    pub(crate) fn new(context: CliContext, target: BytecodeTarget) -> Self {
        let package_name = bytecode_package_name(&context);

        Self {
            key: BytecodeTargetKey::new(&context, &target),
            context,
            package_name,
        }
    }
}

pub(crate) fn bytecode_package_name(context: &CliContext) -> String {
    if context.package_path == "." {
        context
            .package_root
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("package")
            .to_string()
    } else {
        context.package_path.clone()
    }
}
