use ratatui::crossterm::event::KeyModifiers;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub(crate) fn split_lines(contents: &str) -> Vec<String> {
    let mut lines = contents
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(crate) fn editable_char_modifiers(modifiers: KeyModifiers) -> bool {
    !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

pub(crate) fn char_len(value: &str) -> usize {
    value.chars().count()
}

pub(crate) fn char_to_byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(value.len())
}

pub(crate) fn styled_text_segments<const N: usize>(
    segments: [(&str, Style); N],
) -> Vec<Line<'static>> {
    let mut lines = vec![Vec::new()];
    for (text, style) in segments {
        append_styled_text(&mut lines, text, style);
    }
    lines.into_iter().map(Line::from).collect()
}

fn append_styled_text(lines: &mut Vec<Vec<Span<'static>>>, text: &str, style: Style) {
    for (index, part) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(Vec::new());
        }
        if !part.is_empty()
            && let Some(line) = lines.last_mut()
        {
            line.push(Span::styled(part.to_string(), style));
        }
    }
}
