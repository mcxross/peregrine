//! Transcript/history cells for the Peregrine TUI.
//!
//! A `HistoryCell` is the unit of display in the conversation UI, representing both committed
//! transcript entries and, transiently, an in-flight active cell that can mutate in place while
//! streaming.
//!
//! The transcript overlay (`Ctrl+T`) appends a cached live tail derived from the active cell, and
//! that cached tail is refreshed based on an active-cell cache key. Cells that change based on
//! elapsed time expose `transcript_animation_tick()`, and code that mutates the active cell in place
//! bumps the active-cell revision tracked by `ChatWidget`, so the cache key changes whenever the
//! rendered transcript output can change.

use crate::agent::diff_model::FileChange;
use crate::agent::diff_render::create_diff_summary;
use crate::agent::diff_render::display_path_for;
use crate::agent::exec_cell::CommandOutput;
use crate::agent::exec_cell::OutputLinesParams;
use crate::agent::exec_cell::TOOL_CALL_MAX_LINES;
use crate::agent::exec_cell::output_lines;
use crate::agent::exec_command::relativize_to_home;
use crate::agent::exec_command::strip_bash_lc_and_escape;
use crate::agent::legacy_core::config::Config;
use crate::agent::live_wrap::take_prefix_by_width;
use crate::agent::markdown::append_markdown;
use crate::agent::motion::MotionMode;
use crate::agent::motion::ReducedMotionIndicator;
use crate::agent::motion::activity_indicator;
use crate::agent::render::line_utils::line_to_static;
use crate::agent::render::line_utils::prefix_lines;
use crate::agent::render::line_utils::push_owned_lines;
use crate::agent::render::renderable::Renderable;
use crate::agent::session_state::ThreadSessionState;
use crate::agent::style::proposed_plan_style;
use crate::agent::style::user_message_style;
use crate::agent::terminal_hyperlinks::HyperlinkLine;
use crate::agent::terminal_hyperlinks::mark_buffer_hyperlinks;
use crate::agent::terminal_hyperlinks::plain_hyperlink_lines;
use crate::agent::terminal_hyperlinks::prefix_hyperlink_lines;
use crate::agent::terminal_hyperlinks::visible_lines;
#[cfg(test)]
use crate::agent::test_support::PathBufExt;
#[cfg(test)]
use crate::agent::test_support::test_path_buf;
use crate::agent::text_formatting::format_and_truncate_tool_result;
use crate::agent::text_formatting::truncate_text;
use crate::agent::tooltips;
use crate::agent::ui_consts::LIVE_PREFIX_COLS;
use crate::agent::update_action::UpdateAction;
use crate::agent::version::CODEX_CLI_VERSION;
use crate::agent::wrapping::RtOptions;
use crate::agent::wrapping::adaptive_wrap_line;
use crate::agent::wrapping::adaptive_wrap_lines;
use base64::Engine;
use codex_otel::RuntimeMetricsSummary;
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use codex_utils_cli::format_env_display;
use image::DynamicImage;
use image::ImageReader;
use peregrine_app_server_protocol::AskForApproval;
use peregrine_app_server_protocol::McpAuthStatus;
use peregrine_app_server_protocol::McpServerStatus;
use peregrine_app_server_protocol::McpServerStatusDetail;
use peregrine_app_server_protocol::ToolRequestUserInputAnswer;
use peregrine_app_server_protocol::ToolRequestUserInputQuestion;
use peregrine_app_server_protocol::WebSearchAction;
#[cfg(test)]
use peregrine_config::types::McpServerTransportConfig;
use peregrine_types::account::PlanType;
use peregrine_types::approvals::ExecPolicyAmendment;
use peregrine_types::approvals::NetworkPolicyAmendment;
#[cfg(test)]
use peregrine_types::mcp::Resource;
#[cfg(test)]
use peregrine_types::mcp::ResourceTemplate;
use peregrine_types::models::ManagedFileSystemPermissions;
use peregrine_types::models::PermissionProfile;
use peregrine_types::models::local_image_label_text;
use peregrine_types::openai_models::ReasoningEffort as ReasoningEffortConfig;
use peregrine_types::permissions::NetworkSandboxPolicy;
use peregrine_types::plan_tool::PlanItemArg;
use peregrine_types::plan_tool::StepStatus;
use peregrine_types::plan_tool::UpdatePlanArgs;
use peregrine_types::user_input::TextElement;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::any::Any;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tracing::error;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use url::Url;

const RAW_DIFF_SUMMARY_WIDTH: usize = 10_000;
const RAW_TOOL_OUTPUT_WIDTH: usize = 10_000;

#[cfg(test)]
fn qualified_mcp_tool_name_prefix(server_name: &str) -> String {
    let qualified = format!("mcp__{server_name}__");
    let mut sanitized = String::with_capacity(qualified.len());
    for c in qualified.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }
    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized
    }
}

mod approvals;
mod base;
mod exec;
mod hook_cell;
mod mcp;
mod messages;
mod notices;
mod patches;
mod plans;
mod request_user_input;
mod search;
mod separators;
mod session;

pub(crate) use approvals::*;
pub(crate) use base::*;
pub(crate) use exec::*;
pub(crate) use hook_cell::HookCell;
pub(crate) use hook_cell::new_active_hook_cell;
pub(crate) use hook_cell::new_completed_hook_cell;
pub(crate) use mcp::*;
pub(crate) use messages::*;
pub(crate) use notices::*;
pub(crate) use patches::*;
pub(crate) use plans::*;
pub(crate) use request_user_input::*;
pub(crate) use search::*;
pub(crate) use separators::*;
pub(crate) use session::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistoryRenderMode {
    Rich,
    Raw,
}

pub(crate) fn raw_lines_from_source(source: &str) -> Vec<Line<'static>> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut parts = source.split('\n').collect::<Vec<_>>();
    if source.ends_with('\n') {
        parts.pop();
    }

    parts
        .into_iter()
        .map(|line| Line::from(line.to_string()))
        .collect()
}

pub(crate) fn plain_lines(lines: impl IntoIterator<Item = Line<'static>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let text = line
                .spans
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<String>();
            Line::from(text)
        })
        .collect()
}

/// A single renderable unit of conversation history.
///
/// Each cell produces logical `Line`s and reports how many viewport
/// rows those lines occupy at a given terminal width. The default
/// height implementations use `Paragraph::wrap` to account for lines
/// that overflow the viewport width (e.g. long URLs that are kept
/// intact by adaptive wrapping). Concrete types only need to override
/// heights when they apply additional layout logic beyond what
/// `Paragraph::line_count` captures.
pub(crate) trait HistoryCell: std::fmt::Debug + Send + Sync + Any {
    /// Returns the logical lines for the main chat viewport.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;

    /// Returns copy-friendly plain logical lines for raw scrollback mode.
    fn raw_lines(&self) -> Vec<Line<'static>>;

    /// Returns rich visible lines plus terminal hyperlink metadata.
    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        plain_hyperlink_lines(self.display_lines(width))
    }

    fn display_lines_for_mode(&self, width: u16, mode: HistoryRenderMode) -> Vec<Line<'static>> {
        match mode {
            HistoryRenderMode::Rich => visible_lines(self.display_hyperlink_lines(width)),
            HistoryRenderMode::Raw => self.raw_lines(),
        }
    }

    fn display_hyperlink_lines_for_mode(
        &self,
        width: u16,
        mode: HistoryRenderMode,
    ) -> Vec<HyperlinkLine> {
        match mode {
            HistoryRenderMode::Rich => self.display_hyperlink_lines(width),
            HistoryRenderMode::Raw => plain_hyperlink_lines(self.raw_lines()),
        }
    }

    /// Returns the number of viewport rows needed to render this cell.
    ///
    /// The default delegates to `Paragraph::line_count` with
    /// `Wrap { trim: false }`, which measures the actual row count after
    /// ratatui's viewport-level character wrapping. This is critical
    /// for lines containing URL-like tokens that are wider than the
    /// terminal — the logical line count would undercount.
    fn desired_height(&self, width: u16) -> u16 {
        self.desired_height_for_mode(width, HistoryRenderMode::Rich)
    }

    fn desired_height_for_mode(&self, width: u16, mode: HistoryRenderMode) -> u16 {
        Paragraph::new(Text::from(self.display_lines_for_mode(width, mode)))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    /// Returns lines for the transcript overlay (`Ctrl+T`).
    ///
    /// Defaults to `display_lines`. Override when the transcript
    /// representation differs (e.g. `ExecCell` shows all calls with
    /// `$`-prefixed commands and exit status).
    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }

    /// Returns transcript-overlay lines plus terminal hyperlink metadata.
    ///
    /// Defaults to the plain transcript representation because some cells render different
    /// display and transcript content. Rich cells whose transcript mirrors their display should
    /// delegate to `display_hyperlink_lines`.
    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        plain_hyperlink_lines(self.transcript_lines(width))
    }

    /// Returns the number of viewport rows for the transcript overlay.
    ///
    /// Uses the same `Paragraph::line_count` measurement as
    /// `desired_height`. Contains a workaround for a ratatui bug where
    /// a single whitespace-only line reports 2 rows instead of 1.
    fn desired_transcript_height(&self, width: u16) -> u16 {
        let lines = visible_lines(self.transcript_hyperlink_lines(width));
        // Workaround: ratatui's line_count returns 2 for a single
        // whitespace-only line. Clamp to 1 in that case.
        if let [line] = &lines[..]
            && line
                .spans
                .iter()
                .all(|s| s.content.chars().all(char::is_whitespace))
        {
            return 1;
        }

        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    fn is_stream_continuation(&self) -> bool {
        false
    }

    /// Returns a coarse "animation tick" when transcript output is time-dependent.
    ///
    /// The transcript overlay caches the rendered output of the in-flight active cell, so cells
    /// that include time-based UI (spinner, shimmer, etc.) should return a tick that changes over
    /// time to signal that the cached tail should be recomputed. Returning `None` means the
    /// transcript lines are stable, while returning `Some(tick)` during an in-flight animation
    /// allows the overlay to keep up with the main viewport.
    ///
    /// If a cell uses time-based visuals but always returns `None`, `Ctrl+T` can appear "frozen" on
    /// the first rendered frame even though the main viewport is animating.
    fn transcript_animation_tick(&self) -> Option<u64> {
        None
    }
}

impl Renderable for Box<dyn HistoryCell> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let hyperlink_lines = self.display_hyperlink_lines(area.width);
        let lines = visible_lines(hyperlink_lines.clone());
        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        let y = if area.height == 0 {
            0
        } else {
            let overflow = paragraph
                .line_count(area.width)
                .saturating_sub(usize::from(area.height));
            u16::try_from(overflow).unwrap_or(u16::MAX)
        };
        // Active-cell content can reflow dramatically during resize/stream updates. Clear the
        // entire draw area first so stale glyphs from previous frames never linger.
        Clear.render(area, buf);
        paragraph.scroll((y, 0)).render(area, buf);
        mark_buffer_hyperlinks(buf, area, &hyperlink_lines, usize::from(y));
    }
    fn desired_height(&self, width: u16) -> u16 {
        HistoryCell::desired_height(self.as_ref(), width)
    }
}

impl dyn HistoryCell {
    pub(crate) fn as_any(&self) -> &dyn Any {
        self
    }

    pub(crate) fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
