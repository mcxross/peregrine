use super::line_map::build_bytecode_line_map;
use super::types::{BytecodeRequest, BytecodeTargetKey};
use crate::output::CliStatus;
use crate::session;
use crate::sui;
use crate::sui::project::CliContext;
use crate::theme::ThemePalette;
use peregrine_sui_mcp_protocol::{
    BytecodeViewResponse, MoveBytecodeModuleView, MoveBytecodeSourceSpan,
    PackageArgs as McpPackageArgs, tool_name,
};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::super::{PAGE_SIZE, char_len, styled_text_segments, usize_to_u16_saturating};

#[derive(Debug, Clone)]
pub(crate) struct BytecodeSession {
    pub(crate) key: BytecodeTargetKey,
    pub(crate) package_name: String,
    pub(crate) viewer: OwnedBytecodeView,
    pub(crate) current_line: u16,
    pub(crate) current_column: u16,
    pub(crate) horizontal_scroll: u16,
    pub(crate) viewport_width: u16,
}

impl BytecodeSession {
    pub(crate) fn load(request: BytecodeRequest) -> Result<Self, String> {
        let build = sui::runners::run_build(&request.context);
        if build.status != CliStatus::Passed {
            return Err(build
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.clone())
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| {
                    "Compilation failed; fix package errors and try again.".to_string()
                }));
        }
        let response = session::McpToolClient::call_blocking::<_, BytecodeViewResponse>(
            &request.context.project_root,
            tool_name::BYTECODE_VIEW,
            &McpPackageArgs {
                project_root: Some(request.context.project_root.display().to_string()),
                package_path: Some(request.context.package_path.clone()),
            },
        )?;
        let compiled_package_name = response.bytecode.package_name;
        let module = response
            .bytecode
            .modules
            .into_iter()
            .find(|module| !module.is_dependency && module.name == request.key.module_name)
            .ok_or_else(|| {
                format!(
                    "Built package `{compiled_package_name}` but could not find module `{}`.",
                    request.key.module_name
                )
            })?;
        let viewer = OwnedBytecodeView::new(&module, &request.key.source_path)?;

        Ok(Self {
            key: request.key,
            package_name: compiled_package_name,
            viewer,
            current_line: 0,
            current_column: 0,
            horizontal_scroll: 0,
            viewport_width: 1,
        })
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.move_line_up(1),
            KeyCode::Down => self.move_line_down(1),
            KeyCode::PageUp => self.move_line_up(PAGE_SIZE as u16),
            KeyCode::PageDown => self.move_line_down(PAGE_SIZE as u16),
            KeyCode::Left => {
                self.current_column = self.current_column.saturating_sub(1);
                self.ensure_column_visible();
            }
            KeyCode::Right => {
                self.current_column = self
                    .viewer
                    .bound_column(self.current_line, self.current_column.saturating_add(1));
                self.ensure_column_visible();
            }
            KeyCode::Home => {
                self.current_column = 0;
                self.ensure_column_visible();
            }
            KeyCode::End => {
                self.current_column = self.viewer.bound_column(self.current_line, u16::MAX);
                self.ensure_column_visible();
            }
            _ => {}
        }
    }

    pub(crate) fn scroll_vertical(&mut self, down: bool, amount: u16) {
        if down {
            self.move_line_down(amount);
        } else {
            self.move_line_up(amount);
        }
    }

    pub(crate) fn scroll_horizontal(&mut self, right: bool, amount: u16) {
        if right {
            self.horizontal_scroll = self
                .horizontal_scroll
                .saturating_add(amount)
                .min(self.max_horizontal_scroll());
        } else {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
        }
    }

    pub(crate) fn move_line_up(&mut self, amount: u16) {
        self.current_line = self.current_line.saturating_sub(amount);
        self.current_column = self
            .viewer
            .bound_column(self.current_line, self.current_column);
        self.ensure_column_visible();
    }

    pub(crate) fn move_line_down(&mut self, amount: u16) {
        self.current_line = self
            .viewer
            .bound_line(self.current_line.saturating_add(amount));
        self.current_column = self
            .viewer
            .bound_column(self.current_line, self.current_column);
        self.ensure_column_visible();
    }

    pub(crate) fn set_viewport_width(&mut self, width: u16) {
        self.viewport_width = width.max(1);
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    pub(crate) fn max_horizontal_scroll(&self) -> u16 {
        self.viewer
            .max_bytecode_width()
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }

    pub(crate) fn ensure_column_visible(&mut self) {
        if self.current_column < self.horizontal_scroll {
            self.horizontal_scroll = self.current_column;
        } else if self.current_column >= self.horizontal_scroll.saturating_add(self.viewport_width)
        {
            self.horizontal_scroll = self
                .current_column
                .saturating_add(1)
                .saturating_sub(self.viewport_width);
        }
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    pub(crate) fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        palette: ThemePalette,
        focused: bool,
        light_theme: bool,
    ) {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        let inner_height = panes[0].height.saturating_sub(2);
        self.set_viewport_width(panes[0].width.saturating_sub(2));
        let scroll = if inner_height == 0 {
            0
        } else {
            self.current_line.saturating_sub(inner_height / 2)
        };
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
        let selected_style = Style::default()
            .fg(palette.fg)
            .bg(palette.selection)
            .add_modifier(Modifier::BOLD);
        let bytecode_lines =
            self.viewer
                .bytecode_lines(scroll, inner_height, self.current_line, selected_style);
        let source_lines =
            self.viewer
                .source_lines(self.current_line, self.current_column, palette, light_theme);

        let bytecode = Paragraph::new(bytecode_lines)
            .style(base_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "Bytecode - {}::{}",
                        self.package_name, self.key.module_name
                    ))
                    .style(base_style)
                    .border_style(border_style)
                    .title_style(title_style),
            )
            .scroll((scroll, self.horizontal_scroll));
        frame.render_widget(bytecode, panes[0]);

        let source = Paragraph::new(source_lines).style(base_style).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Source Code")
                .style(base_style)
                .border_style(border_style)
                .title_style(title_style),
        );
        frame.render_widget(source, panes[1]);

        if focused && inner_height > 0 {
            let y = self
                .current_line
                .saturating_sub(scroll)
                .min(inner_height - 1);
            let x = self.current_column.saturating_sub(self.horizontal_scroll);
            if self.current_column >= self.horizontal_scroll && x < self.viewport_width {
                frame.set_cursor_position(Position::new(panes[0].x + 1 + x, panes[0].y + 1 + y));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedBytecodeView {
    bytecode_lines: Vec<String>,
    line_map: HashMap<usize, MoveBytecodeSourceSpan>,
    source_code: String,
}

impl OwnedBytecodeView {
    pub(crate) fn new(module: &MoveBytecodeModuleView, source_path: &Path) -> Result<Self, String> {
        let source_code = fs::read_to_string(source_path)
            .map_err(|error| format!("Could not read {}: {error}", source_path.display()))?;
        let bytecode_lines = module
            .disassembly
            .lines()
            .map(|line| line.replace('\t', "    "))
            .collect::<Vec<_>>();
        let line_map = build_bytecode_line_map(module, &bytecode_lines)?;

        Ok(Self {
            bytecode_lines,
            line_map,
            source_code,
        })
    }

    pub(crate) fn bytecode_lines(
        &self,
        scroll: u16,
        height: u16,
        current_line: u16,
        selected_style: Style,
    ) -> Vec<Line<'static>> {
        self.bytecode_lines
            .iter()
            .skip(scroll as usize)
            .take(height as usize)
            .enumerate()
            .map(|(visible_index, line)| {
                let line_index = scroll as usize + visible_index;
                if line_index == current_line as usize {
                    Line::styled(line.clone(), selected_style)
                } else {
                    Line::from(line.clone())
                }
            })
            .collect()
    }

    pub(crate) fn max_bytecode_width(&self) -> u16 {
        self.bytecode_lines
            .iter()
            .map(|line| usize_to_u16_saturating(char_len(line)))
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn source_lines(
        &self,
        line_number: u16,
        _column_number: u16,
        palette: ThemePalette,
        light_theme: bool,
    ) -> Vec<Line<'static>> {
        let base_style = Style::default().fg(palette.syntax.text).bg(palette.bg);
        let highlight_style = Style::default()
            .fg(if light_theme { palette.bg } else { palette.fg })
            .bg(palette.warning)
            .add_modifier(Modifier::BOLD);
        let Some(info) = self.line_map.get(&(line_number as usize)) else {
            return self
                .source_code
                .lines()
                .map(|line| Line::styled(line.to_string(), base_style))
                .collect();
        };
        let start = info.start_byte as usize;
        let end = info.end_byte as usize;
        let source_len = self.source_code.len();
        if start > end || end > source_len {
            return vec![Line::styled(
                "The bytecode source location is out of sync with the source file.",
                Style::default().fg(palette.muted).bg(palette.bg),
            )];
        }
        let context_start = start.saturating_sub(1000);
        let context_end = end.saturating_add(1000).min(source_len);

        styled_text_segments([
            (&self.source_code[context_start..start], base_style),
            (&self.source_code[start..end], highlight_style),
            (&self.source_code[end..context_end], base_style),
        ])
    }

    pub(crate) fn bound_line(&self, line_number: u16) -> u16 {
        let last = self.bytecode_lines.len().saturating_sub(1) as u16;
        line_number.min(last)
    }

    pub(crate) fn bound_column(&self, line_number: u16, column_number: u16) -> u16 {
        let line = self
            .bytecode_lines
            .get(line_number as usize)
            .map(String::as_str)
            .unwrap_or_default();
        column_number.min(char_len(line) as u16)
    }
}
