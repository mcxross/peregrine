//! Color themes for the Peregrine workbench.

pub mod palette;
mod state;
mod theme;
pub mod widgets;

pub use palette::{GraphPalette, SyntaxPalette, ThemePalette};
pub use state::{ThemeState, shared_theme_state};
pub use theme::{Theme, ThemeName};
pub use widgets::ThemePicker;
