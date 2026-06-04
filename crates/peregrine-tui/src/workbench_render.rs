use crate::agent::{highlight_code_for_path_to_lines, render_markdown_text_with_width_and_cwd};
use crate::theme::ThemePalette;
use move_command_line_common::files::FileHash;
use move_compiler::editions::Edition;
use move_compiler::parser::lexer::{Lexer, Tok};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkbenchRenderMode {
    Move,
    MarkdownPreview,
    CommonSyntax,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedWorkbenchDocument {
    pub mode: WorkbenchRenderMode,
    pub lines: Vec<Line<'static>>,
    pub show_gutter: bool,
    pub show_cursor: bool,
}

#[derive(Clone, Copy, Debug)]
struct StyleRange {
    start: usize,
    end: usize,
    style: Style,
}

#[derive(Clone, Debug)]
struct MoveToken {
    kind: Tok,
    start: usize,
    end: usize,
    text: String,
}

pub(crate) fn render_workbench_document(
    source: &str,
    path: Option<&Path>,
    palette: ThemePalette,
    markdown_preview: bool,
    width: usize,
    cwd: Option<&Path>,
) -> RenderedWorkbenchDocument {
    let Some(path) = path else {
        return RenderedWorkbenchDocument::plain(source, palette, WorkbenchRenderMode::Plain);
    };

    if is_markdown_path(path) && markdown_preview {
        let lines = render_markdown_text_with_width_and_cwd(source, Some(width), cwd).lines;
        return RenderedWorkbenchDocument {
            mode: WorkbenchRenderMode::MarkdownPreview,
            lines,
            show_gutter: false,
            show_cursor: false,
        };
    }

    if is_move_path(path) {
        return RenderedWorkbenchDocument {
            mode: WorkbenchRenderMode::Move,
            lines: highlight_move_source(source, palette),
            show_gutter: true,
            show_cursor: true,
        };
    }

    if let Some(lines) = highlight_code_for_path_to_lines(source, path) {
        return RenderedWorkbenchDocument {
            mode: WorkbenchRenderMode::CommonSyntax,
            lines,
            show_gutter: true,
            show_cursor: true,
        };
    }

    RenderedWorkbenchDocument::plain(source, palette, WorkbenchRenderMode::Plain)
}

impl RenderedWorkbenchDocument {
    fn plain(source: &str, palette: ThemePalette, mode: WorkbenchRenderMode) -> Self {
        Self {
            mode,
            lines: plain_lines(source, base_style(palette)),
            show_gutter: true,
            show_cursor: true,
        }
    }
}

fn is_move_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("move"))
}

pub(crate) fn is_markdown_path(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
    {
        return true;
    }

    path.file_name()
        .and_then(|file_name| file_name.to_str())
        .is_some_and(|file_name| file_name.eq_ignore_ascii_case("readme"))
}

fn highlight_move_source(source: &str, palette: ThemePalette) -> Vec<Line<'static>> {
    let mut ranges = comment_ranges(source)
        .into_iter()
        .map(|(start, end)| StyleRange {
            start,
            end,
            style: Style::default().fg(palette.syntax.comment),
        })
        .collect::<Vec<_>>();

    let tokens = lex_move_tokens(source);
    let mut previous_kind = None;
    let mut attribute_depth = 0usize;
    let mut pending_attribute = false;

    for (index, token) in tokens.iter().enumerate() {
        let next_kind = tokens.get(index + 1).map(|next| next.kind);
        let style = move_token_style(
            token,
            previous_kind,
            next_kind,
            palette,
            &mut attribute_depth,
            &mut pending_attribute,
        );
        ranges.push(StyleRange {
            start: token.start,
            end: token.end,
            style,
        });
        previous_kind = Some(token.kind);
    }

    ranges.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
    styled_lines_from_ranges(source, &ranges, base_style(palette))
}

fn lex_move_tokens(source: &str) -> Vec<MoveToken> {
    let mut lexer = Lexer::new(source, FileHash::new(source), Edition::default());
    let mut tokens = Vec::new();

    loop {
        let _ = lexer.advance();
        let kind = lexer.peek();
        if kind == Tok::EOF {
            break;
        }
        let start = lexer.start_loc();
        let text = lexer.content();
        let end = start.saturating_add(text.len());
        if start == end {
            break;
        }
        tokens.push(MoveToken {
            kind,
            start,
            end,
            text: text.to_string(),
        });
    }

    tokens
}

fn move_token_style(
    token: &MoveToken,
    previous_kind: Option<Tok>,
    next_kind: Option<Tok>,
    palette: ThemePalette,
    attribute_depth: &mut usize,
    pending_attribute: &mut bool,
) -> Style {
    if *attribute_depth > 0 {
        update_attribute_state(token.kind, attribute_depth, pending_attribute);
        return Style::default().fg(palette.syntax.macro_name);
    }

    if *pending_attribute && token.kind == Tok::LBracket {
        *attribute_depth = 1;
        *pending_attribute = false;
        return Style::default().fg(palette.syntax.macro_name);
    }

    if token.kind == Tok::NumSign {
        *pending_attribute = true;
        return Style::default().fg(palette.syntax.macro_name);
    }

    *pending_attribute = false;

    let syntax = palette.syntax;
    let color = match token.kind {
        Tok::Abort
        | Tok::Acquires
        | Tok::As
        | Tok::Break
        | Tok::Continue
        | Tok::Copy
        | Tok::Else
        | Tok::For
        | Tok::Friend
        | Tok::If
        | Tok::Invariant
        | Tok::Let
        | Tok::Loop
        | Tok::Match
        | Tok::Module
        | Tok::Move
        | Tok::Mut
        | Tok::Native
        | Tok::Public
        | Tok::Return
        | Tok::Spec
        | Tok::Use
        | Tok::While => syntax.keyword,
        Tok::Const | Tok::Enum | Tok::Struct | Tok::Type => syntax.type_name,
        Tok::Fun => syntax.function,
        Tok::True | Tok::False | Tok::NumTypedValue | Tok::NumValue => syntax.number,
        Tok::StringValue => syntax.string,
        Tok::AtSign => syntax.macro_name,
        Tok::Identifier | Tok::RestrictedIdentifier | Tok::SyntaxIdentifier | Tok::BlockLabel => {
            if is_security_sensitive_identifier(&token.text) {
                syntax.security_sensitive
            } else if matches!(
                previous_kind,
                Some(Tok::Module | Tok::Struct | Tok::Enum | Tok::Type)
            ) || is_move_type_identifier(&token.text)
            {
                syntax.type_name
            } else if matches!(previous_kind, Some(Tok::Fun))
                || matches!(next_kind, Some(Tok::LParen))
            {
                syntax.function
            } else if starts_with_uppercase(&token.text) {
                syntax.type_name
            } else {
                syntax.variable
            }
        }
        Tok::EOF => syntax.text,
        _ => syntax.operator,
    };
    Style::default().fg(color)
}

fn update_attribute_state(kind: Tok, attribute_depth: &mut usize, pending_attribute: &mut bool) {
    match kind {
        Tok::LBracket => *attribute_depth = attribute_depth.saturating_add(1),
        Tok::RBracket => {
            *attribute_depth = attribute_depth.saturating_sub(1);
            if *attribute_depth == 0 {
                *pending_attribute = false;
            }
        }
        _ => {}
    }
}

fn is_move_type_identifier(text: &str) -> bool {
    matches!(
        text,
        "address" | "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "u256" | "vector" | "Self"
    )
}

fn is_security_sensitive_identifier(text: &str) -> bool {
    matches!(
        text,
        "signer"
            | "UID"
            | "ID"
            | "TxContext"
            | "Receiving"
            | "transfer"
            | "public_transfer"
            | "party_transfer"
            | "move_from"
            | "move_to"
            | "borrow_global"
            | "borrow_global_mut"
    )
}

fn starts_with_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
}

fn comment_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut index = 0;

    while index < source.len() {
        let rest = &source[index..];
        if rest.starts_with("//") {
            let end = rest
                .find('\n')
                .map_or(source.len(), |offset| index + offset);
            ranges.push((index, end));
            index = end;
            continue;
        }
        if rest.starts_with("/*") {
            let start = index;
            index = index.saturating_add(2);
            let mut depth = 1usize;
            while index < source.len() && depth > 0 {
                let block_rest = &source[index..];
                if block_rest.starts_with("/*") {
                    depth = depth.saturating_add(1);
                    index = index.saturating_add(2);
                } else if block_rest.starts_with("*/") {
                    depth = depth.saturating_sub(1);
                    index = index.saturating_add(2);
                } else {
                    index = advance_one_char(source, index);
                }
            }
            ranges.push((start, index.min(source.len())));
            continue;
        }
        if rest.starts_with('"') {
            index = skip_string(source, index.saturating_add(1));
            continue;
        }
        if (rest.starts_with("b\"") || rest.starts_with("x\""))
            && is_string_prefix_boundary(source, index)
        {
            index = skip_string(source, index.saturating_add(2));
            continue;
        }
        index = advance_one_char(source, index);
    }

    ranges
}

fn is_string_prefix_boundary(source: &str, index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let previous = source[..index].chars().next_back();
    !previous.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn skip_string(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        index = index.saturating_add(ch.len_utf8());
        if ch == '\\' {
            index = advance_one_char(source, index);
        } else if ch == '"' {
            break;
        }
    }
    index
}

fn advance_one_char(source: &str, index: usize) -> usize {
    source[index..]
        .chars()
        .next()
        .map_or(source.len(), |ch| index.saturating_add(ch.len_utf8()))
}

fn styled_lines_from_ranges(
    source: &str,
    ranges: &[StyleRange],
    base_style: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut range_index = 0usize;

    for line_text in source.split('\n') {
        let line_end = line_start.saturating_add(line_text.len());
        while range_index < ranges.len() && ranges[range_index].end <= line_start {
            range_index = range_index.saturating_add(1);
        }
        lines.push(Line::from(spans_for_line(
            source,
            line_start,
            line_end,
            ranges,
            range_index,
            base_style,
        )));
        line_start = line_end.saturating_add(1);
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(String::new(), base_style)));
    }

    lines
}

fn spans_for_line(
    source: &str,
    line_start: usize,
    line_end: usize,
    ranges: &[StyleRange],
    range_index: usize,
    base_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut cursor = line_start;
    let mut local_index = range_index;

    while local_index < ranges.len() && ranges[local_index].start < line_end {
        let range = ranges[local_index];
        if range.end <= cursor {
            local_index = local_index.saturating_add(1);
            continue;
        }
        if range.start > cursor {
            push_span(
                source,
                &mut spans,
                cursor,
                range.start.min(line_end),
                base_style,
            );
        }
        let styled_start = range.start.max(cursor);
        let styled_end = range.end.min(line_end);
        push_span(source, &mut spans, styled_start, styled_end, range.style);
        cursor = styled_end;
        if range.end <= line_end {
            local_index = local_index.saturating_add(1);
        } else {
            break;
        }
    }

    if cursor < line_end {
        push_span(source, &mut spans, cursor, line_end, base_style);
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base_style));
    }
    spans
}

fn push_span(source: &str, spans: &mut Vec<Span<'static>>, start: usize, end: usize, style: Style) {
    if start >= end {
        return;
    }
    spans.push(Span::styled(source[start..end].to_string(), style));
}

fn plain_lines(source: &str, style: Style) -> Vec<Line<'static>> {
    source
        .split('\n')
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

fn base_style(palette: ThemePalette) -> Style {
    Style::default().fg(palette.syntax.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeName;
    use ratatui::style::Color;

    fn palette() -> ThemePalette {
        ThemeName::PeregrineNight.palette()
    }

    fn span_text(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn foregrounds_for_text(lines: &[Line<'static>], needle: &str) -> Vec<Color> {
        lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .filter(|span| span.content.contains(needle))
            .filter_map(|span| span.style.fg)
            .collect()
    }

    #[test]
    fn move_rendering_highlights_core_token_classes() {
        let palette = palette();
        let source = r#"module savings::vault {
    /// A doc comment.
    #[test]
    public fun deposit(ctx: &mut TxContext, amount: u64): ID {
        let label = "vault";
        let enabled = true;
        transfer::public_transfer(label, @0x1);
    }
}"#;
        let rendered = render_workbench_document(
            source,
            Some(Path::new("sources/vault.move")),
            palette,
            false,
            80,
            None,
        );

        assert_eq!(rendered.mode, WorkbenchRenderMode::Move);
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "module"),
            vec![palette.syntax.keyword]
        );
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "deposit"),
            vec![palette.syntax.function]
        );
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "TxContext"),
            vec![palette.syntax.security_sensitive]
        );
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "u64"),
            vec![palette.syntax.type_name]
        );
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "\"vault\""),
            vec![palette.syntax.string]
        );
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "true"),
            vec![palette.syntax.number]
        );
        assert!(foregrounds_for_text(&rendered.lines, "#").contains(&palette.syntax.macro_name));
        assert!(foregrounds_for_text(&rendered.lines, "///").contains(&palette.syntax.comment));
    }

    #[test]
    fn move_rendering_recovers_from_partial_source() {
        let source =
            "module m::broken {\n    fun f() { let label = \"unterminated;\n    /* comment";
        let rendered = render_workbench_document(
            source,
            Some(Path::new("broken.move")),
            palette(),
            false,
            80,
            None,
        );
        assert_eq!(rendered.mode, WorkbenchRenderMode::Move);
        assert_eq!(span_text(&rendered.lines[0]), "module m::broken {");
        let last_line = rendered
            .lines
            .last()
            .unwrap_or_else(|| panic!("expected at least one rendered line"));
        assert!(span_text(last_line).contains("/* comment"));
    }

    #[test]
    fn common_language_uses_syntect_path_resolution() {
        let rendered = render_workbench_document(
            "fn main() { let answer = 42; }\n",
            Some(Path::new("main.rs")),
            palette(),
            false,
            80,
            None,
        );
        assert_eq!(rendered.mode, WorkbenchRenderMode::CommonSyntax);
        assert!(
            rendered
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.style.fg.is_some())
        );
    }

    #[test]
    fn unknown_extension_renders_plain() {
        let palette = palette();
        let rendered = render_workbench_document(
            "plain text",
            Some(Path::new("file.unknown-language")),
            palette,
            false,
            80,
            None,
        );
        assert_eq!(rendered.mode, WorkbenchRenderMode::Plain);
        assert_eq!(
            foregrounds_for_text(&rendered.lines, "plain text"),
            vec![palette.syntax.text]
        );
    }

    #[test]
    fn markdown_preview_renders_and_hides_source_affordances() {
        let rendered = render_workbench_document(
            "# Title\n\nbody",
            Some(Path::new("README.md")),
            palette(),
            true,
            80,
            None,
        );
        assert_eq!(rendered.mode, WorkbenchRenderMode::MarkdownPreview);
        assert!(!rendered.show_gutter);
        assert!(!rendered.show_cursor);
        assert!(
            rendered.lines[0]
                .spans
                .iter()
                .any(|span| span.content.contains("Title"))
        );
    }

    #[test]
    fn markdown_raw_editing_uses_common_syntax() {
        let rendered = render_workbench_document(
            "# Title",
            Some(Path::new("README.md")),
            palette(),
            false,
            80,
            None,
        );
        assert_eq!(rendered.mode, WorkbenchRenderMode::CommonSyntax);
        assert!(rendered.show_gutter);
        assert!(rendered.show_cursor);
        assert_eq!(span_text(&rendered.lines[0]), "# Title");
    }

    #[test]
    fn theme_palette_changes_move_styles() {
        let dark = ThemeName::PeregrineNight.palette();
        let light = ThemeName::EvidenceLight.palette();
        let source = "module m::a { public fun f() {} }";
        let dark_rendered =
            render_workbench_document(source, Some(Path::new("a.move")), dark, false, 80, None);
        let light_rendered =
            render_workbench_document(source, Some(Path::new("a.move")), light, false, 80, None);
        assert_ne!(
            foregrounds_for_text(&dark_rendered.lines, "module"),
            foregrounds_for_text(&light_rendered.lines, "module")
        );
    }
}
