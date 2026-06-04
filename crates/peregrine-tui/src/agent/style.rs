use crate::agent::color::blend;
use crate::agent::color::is_light;
use crate::agent::terminal_palette::StdoutColorLevel;
use crate::agent::terminal_palette::best_color;
use crate::agent::terminal_palette::rgb_color;
use crate::theme::ThemeName;
use crate::theme::ThemePalette;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use std::str::FromStr;
use std::sync::OnceLock;
use std::sync::RwLock;

const LIGHT_BG_ACCENT_RGB: (u8, u8, u8) = (0, 95, 135);
// Decorative table rules should remain visible without competing with cell content.
const TABLE_SEPARATOR_FG_ALPHA: f32 = 0.20;
static AGENT_THEME: OnceLock<RwLock<ThemeName>> = OnceLock::new();

fn theme_lock() -> &'static RwLock<ThemeName> {
    AGENT_THEME.get_or_init(|| RwLock::new(ThemeName::default()))
}

pub(crate) fn set_agent_theme(name: Option<&str>) -> Option<String> {
    let theme = match name {
        Some(name) => match ThemeName::from_str(name) {
            Ok(theme) => theme,
            Err(_) => {
                *theme_lock().write().expect("agent theme lock poisoned") = ThemeName::default();
                return Some(format!(
                    "Theme \"{name}\" not found. Using the default Peregrine theme."
                ));
            }
        },
        None => ThemeName::default(),
    };

    *theme_lock().write().expect("agent theme lock poisoned") = theme;
    None
}

pub(crate) fn current_theme_name() -> ThemeName {
    *theme_lock().read().expect("agent theme lock poisoned")
}

pub(crate) fn current_palette() -> ThemePalette {
    current_theme_name().palette()
}

pub fn user_message_style() -> Style {
    user_message_style_for_palette(current_palette())
}

pub fn proposed_plan_style() -> Style {
    proposed_plan_style_for_palette(current_palette())
}

/// Returns a low-contrast rule style for separators within markdown tables.
pub(crate) fn table_separator_style() -> Style {
    table_separator_style_for_palette(current_palette())
}

/// Returns the shared accent style for active or selected TUI controls.
pub(crate) fn accent_style() -> Style {
    accent_style_for_palette(current_palette())
}

fn user_message_style_for_palette(palette: ThemePalette) -> Style {
    Style::default().fg(palette.fg).bg(palette.selection)
}

fn proposed_plan_style_for_palette(palette: ThemePalette) -> Style {
    Style::default().fg(palette.fg).bg(palette.selection)
}

fn table_separator_style_for_palette(palette: ThemePalette) -> Style {
    Style::default().fg(palette.muted)
}

fn accent_style_for_palette(palette: ThemePalette) -> Style {
    Style::default().fg(palette.accent).bold()
}

/// Returns the style for a user-authored message using the provided terminal background.
pub fn user_message_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(user_message_bg(bg)),
        None => Style::default(),
    }
}

pub fn proposed_plan_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(proposed_plan_bg(bg)),
        None => Style::default(),
    }
}

/// Returns the shared accent style for the provided terminal background.
pub(crate) fn accent_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    if terminal_bg.is_some_and(is_light) {
        Style::default().fg(best_color(LIGHT_BG_ACCENT_RGB)).bold()
    } else {
        Style::default().fg(Color::Cyan).bold()
    }
}

fn table_separator_style_for(
    terminal_fg: Option<(u8, u8, u8)>,
    terminal_bg: Option<(u8, u8, u8)>,
    color_level: StdoutColorLevel,
) -> Style {
    let (Some(fg), Some(bg)) = (terminal_fg, terminal_bg) else {
        return Style::default().dim();
    };
    let separator_rgb = blend(fg, bg, TABLE_SEPARATOR_FG_ALPHA);
    match color_level {
        StdoutColorLevel::TrueColor => Style::default().fg(rgb_color(separator_rgb)),
        StdoutColorLevel::Ansi256 => Style::default().fg(best_color(separator_rgb)),
        StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown => Style::default().dim(),
    }
}

#[allow(clippy::disallowed_methods)]
pub fn user_message_bg(terminal_bg: (u8, u8, u8)) -> Color {
    let (top, alpha) = if is_light(terminal_bg) {
        ((0, 0, 0), 0.04)
    } else {
        ((255, 255, 255), 0.12)
    };
    best_color(blend(top, terminal_bg, alpha))
}

#[allow(clippy::disallowed_methods)]
pub fn proposed_plan_bg(terminal_bg: (u8, u8, u8)) -> Color {
    user_message_bg(terminal_bg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    #[test]
    fn accent_style_uses_darker_cyan_on_light_backgrounds() {
        let style = accent_style_for(Some((255, 255, 255)));

        assert_eq!(style.fg, Some(best_color(LIGHT_BG_ACCENT_RGB)));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn accent_style_uses_cyan_on_dark_or_unknown_backgrounds() {
        let expected = Style::default().fg(Color::Cyan).bold();

        assert_eq!(accent_style_for(Some((0, 0, 0))), expected);
        assert_eq!(accent_style_for(/*terminal_bg*/ None), expected);
    }

    #[test]
    fn table_separator_blends_toward_dark_background() {
        let style = table_separator_style_for(
            Some((255, 255, 255)),
            Some((0, 0, 0)),
            StdoutColorLevel::TrueColor,
        );

        assert_eq!(style.fg, Some(rgb_color((51, 51, 51))));
    }

    #[test]
    fn table_separator_blends_toward_light_background() {
        let style = table_separator_style_for(
            Some((0, 0, 0)),
            Some((255, 255, 255)),
            StdoutColorLevel::TrueColor,
        );

        assert_eq!(style.fg, Some(rgb_color((204, 204, 204))));
    }

    #[test]
    fn table_separator_dims_when_palette_aware_color_is_unavailable() {
        let expected = Style::default().dim();

        assert_eq!(
            table_separator_style_for(
                Some((255, 255, 255)),
                Some((0, 0, 0)),
                StdoutColorLevel::Ansi16,
            ),
            expected
        );
        assert_eq!(
            table_separator_style_for(
                /*terminal_fg*/ None,
                Some((0, 0, 0)),
                StdoutColorLevel::TrueColor,
            ),
            expected
        );
    }

    #[test]
    fn agent_semantic_styles_use_current_peregrine_palette() {
        let palette = ThemeName::ZeroDay.palette();

        assert_eq!(accent_style_for_palette(palette).fg, Some(palette.accent));
        assert_eq!(
            user_message_style_for_palette(palette).bg,
            Some(palette.selection)
        );
        assert_eq!(
            table_separator_style_for_palette(palette).fg,
            Some(palette.muted)
        );
    }
}
