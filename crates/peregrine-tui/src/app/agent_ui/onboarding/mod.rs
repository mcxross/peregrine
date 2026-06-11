use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub(crate) fn mark_url_hyperlink(buf: &mut Buffer, area: Rect, url: &str) {
    crate::agent::terminal_hyperlinks::mark_url_hyperlink(buf, area, url);
}

pub(crate) fn mark_underlined_hyperlink(buf: &mut Buffer, area: Rect, url: &str) {
    crate::agent::terminal_hyperlinks::mark_underlined_hyperlink(buf, area, url);
}
