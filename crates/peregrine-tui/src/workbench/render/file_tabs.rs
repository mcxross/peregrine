use crate::workbench::prelude::*;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) struct FileTabBar<'a> {
    tabs: Vec<VisibleFileTab<'a>>,
    hidden_before: bool,
    hidden_after: bool,
    focused: bool,
    base_style: Style,
    muted_style: Style,
    active_style: Style,
    accent_style: Style,
    dirty_style: Style,
}

impl<'a> FileTabBar<'a> {
    pub(crate) fn new(
        tabs: Vec<VisibleFileTab<'a>>,
        hidden_before: bool,
        hidden_after: bool,
        focused: bool,
        base_style: Style,
        muted_style: Style,
        active_style: Style,
        accent_style: Style,
        dirty_style: Style,
    ) -> Self {
        Self {
            tabs,
            hidden_before,
            hidden_after,
            focused,
            base_style,
            muted_style,
            active_style,
            accent_style,
            dirty_style,
        }
    }

    pub(crate) fn hit_areas(&self, area: Rect) -> Vec<FileTabHitArea> {
        placements(&self.tabs, self.hidden_before, self.hidden_after, area)
            .flat_map(|placement| match placement {
                FileTabPlacement::Previous(area) => vec![FileTabHitArea {
                    target: FileTabHitTarget::Previous,
                    area,
                }],
                FileTabPlacement::Next(area) => vec![FileTabHitArea {
                    target: FileTabHitTarget::Next,
                    area,
                }],
                FileTabPlacement::Tab { id, area, close } => {
                    let mut areas = Vec::with_capacity(2);
                    if close.width > 0 {
                        areas.push(FileTabHitArea {
                            target: FileTabHitTarget::Close(id),
                            area: close,
                        });
                    }
                    areas.push(FileTabHitArea {
                        target: FileTabHitTarget::Activate(id),
                        area,
                    });
                    areas
                }
            })
            .collect()
    }
}

impl Widget for FileTabBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        buf.set_style(area, self.base_style);
        if self.tabs.is_empty() {
            buf.set_stringn(
                area.x.saturating_add(1),
                area.y,
                "no open files",
                usize::from(area.width.saturating_sub(1)),
                self.muted_style,
            );
            return;
        }

        let mut tab_index = 0;
        for placement in placements(&self.tabs, self.hidden_before, self.hidden_after, area) {
            match placement {
                FileTabPlacement::Previous(control) => {
                    buf.set_stringn(control.x, control.y, " ‹", 2, self.accent_style);
                }
                FileTabPlacement::Next(control) => {
                    buf.set_stringn(control.x, control.y, "› ", 2, self.accent_style);
                }
                FileTabPlacement::Tab { area, .. } => {
                    let tab = self.tabs[tab_index];
                    tab_index += 1;
                    render_tab(
                        tab,
                        area,
                        self.focused,
                        self.base_style,
                        self.muted_style,
                        self.active_style,
                        self.accent_style,
                        self.dirty_style,
                        buf,
                    );
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FileTabPlacement {
    Previous(Rect),
    Tab {
        id: DocumentId,
        area: Rect,
        close: Rect,
    },
    Next(Rect),
}

fn placements<'a>(
    tabs: &'a [VisibleFileTab<'a>],
    hidden_before: bool,
    hidden_after: bool,
    area: Rect,
) -> impl Iterator<Item = FileTabPlacement> + 'a {
    let mut items = Vec::with_capacity(tabs.len().saturating_add(2));
    let mut x = area.x;
    let right = area.right();
    if hidden_before && x < right {
        let width = FILE_TAB_CONTROL_WIDTH.min(right.saturating_sub(x));
        items.push(FileTabPlacement::Previous(Rect::new(x, area.y, width, 1)));
        x = x.saturating_add(width);
    }
    let tab_right = right.saturating_sub(if hidden_after {
        FILE_TAB_CONTROL_WIDTH
    } else {
        0
    });
    for tab in tabs {
        if x >= tab_right {
            break;
        }
        let width = tab.width.min(tab_right.saturating_sub(x));
        let tab_area = Rect::new(x, area.y, width, 1);
        let close = if width >= 4 {
            Rect::new(tab_area.right().saturating_sub(3), area.y, 2, 1)
        } else {
            Rect::default()
        };
        items.push(FileTabPlacement::Tab {
            id: tab.id,
            area: tab_area,
            close,
        });
        x = x.saturating_add(width);
    }
    if hidden_after && right > area.x {
        let width = FILE_TAB_CONTROL_WIDTH.min(area.width);
        items.push(FileTabPlacement::Next(Rect::new(
            right.saturating_sub(width),
            area.y,
            width,
            1,
        )));
    }
    items.into_iter()
}

#[allow(clippy::too_many_arguments)]
fn render_tab(
    tab: VisibleFileTab<'_>,
    area: Rect,
    focused: bool,
    base_style: Style,
    muted_style: Style,
    active_style: Style,
    accent_style: Style,
    dirty_style: Style,
    buf: &mut Buffer,
) {
    if area.width == 0 {
        return;
    }
    let style = if tab.active {
        active_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        base_style
    };
    buf.set_style(area, style);
    let marker = if focused && tab.active { "›" } else { " " };
    buf.set_stringn(area.x, area.y, marker, 1, accent_style);
    let label_width = usize::from(area.width.saturating_sub(6));
    if label_width > 0 {
        let label = truncate_label(tab.label, label_width);
        buf.set_stringn(area.x.saturating_add(2), area.y, label, label_width, style);
    }
    if area.width >= 5 {
        let dirty_x = area.right().saturating_sub(4);
        buf.set_stringn(
            dirty_x,
            area.y,
            if tab.dirty { "*" } else { " " },
            1,
            dirty_style,
        );
        buf.set_stringn(
            area.right().saturating_sub(3),
            area.y,
            "×",
            1,
            if tab.active {
                accent_style
            } else {
                muted_style
            },
        );
        buf.set_stringn(area.right().saturating_sub(1), area.y, "│", 1, muted_style);
    }
}

fn truncate_label(label: &str, width: usize) -> String {
    if UnicodeWidthStr::width(label) <= width {
        return label.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let content_width = width.saturating_sub(1);
    let mut output = String::new();
    let mut used = 0_usize;
    for grapheme in UnicodeSegmentation::graphemes(label, true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if used.saturating_add(grapheme_width) > content_width {
            break;
        }
        output.push_str(grapheme);
        used = used.saturating_add(grapheme_width);
    }
    output.push('…');
    output
}

impl App {
    pub(crate) fn render_file_tabs(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.editor.ensure_active_visible(area.width);
        let hidden_before = self.editor.has_hidden_before();
        let hidden_after = self.editor.has_hidden_after(area.width);
        let tabs = self.editor.visible_tabs(area.width);
        let palette = self.palette();
        let bar = FileTabBar::new(
            tabs,
            hidden_before,
            hidden_after,
            self.focus == FocusPane::FileTabs,
            self.base_style(),
            self.muted_style(),
            self.style_fg(palette.fg),
            self.style_fg(palette.accent),
            self.style_fg(palette.warning),
        );
        self.layout.file_tab_hit_areas = bar.hit_areas(area);
        frame.render_widget(bar, area);
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_label;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn truncation_respects_unicode_display_width() {
        let label = truncate_label("合同.move", 6);
        assert!(UnicodeWidthStr::width(label.as_str()) <= 6);
        assert!(label.ends_with('…'));
    }
}
