//! Optional widgets for theme selection.
//!
//! This module intentionally stays small so applications can build their own
//! theme picker UI around [`ThemeName::all`](super::ThemeName::all).

use super::ThemeName;

/// Minimal state holder for a theme picker widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePicker {
    /// Currently selected theme.
    pub selected: ThemeName,
}

impl ThemePicker {
    /// Create a new theme picker with the given selected theme.
    #[must_use]
    pub const fn new(selected: ThemeName) -> Self {
        Self { selected }
    }

    /// Move selection to the next theme.
    pub fn next(&mut self) {
        self.selected = self.selected.next();
    }

    /// Move selection to the previous theme.
    pub fn prev(&mut self) {
        self.selected = self.selected.prev();
    }
}

impl Default for ThemePicker {
    fn default() -> Self {
        Self::new(ThemeName::default())
    }
}
