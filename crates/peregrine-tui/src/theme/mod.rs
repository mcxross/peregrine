//! Color themes for the Peregrine workbench.

pub mod palette;
mod theme;
pub mod widgets;

pub use palette::{GraphPalette, SyntaxPalette, ThemePalette};
pub use theme::{Theme, ThemeName};
pub use widgets::ThemePicker;
