use super::EditorMode;
use crate::app;
use crate::theme::Theme;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiSettings {
    pub editor_mode: EditorMode,
    pub theme: Theme,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            editor_mode: EditorMode::Standard,
            theme: Theme::default(),
        }
    }
}

pub fn configured_tui_settings() -> TuiSettings {
    let Ok(root) = std::env::current_dir() else {
        return TuiSettings::default();
    };
    app::ApplicationRuntime::load(root)
        .map(|runtime| {
            let ui = runtime.ui();
            TuiSettings {
                editor_mode: ui.editor_mode,
                theme: ui.theme,
            }
        })
        .unwrap_or_default()
}

pub fn load_tui_settings_from_home(home: &Path) -> TuiSettings {
    app::ApplicationRuntime::load_from_home(home.to_path_buf(), home.to_path_buf())
        .map(|runtime| {
            let ui = runtime.ui();
            TuiSettings {
                editor_mode: ui.editor_mode,
                theme: ui.theme,
            }
        })
        .unwrap_or_default()
}

pub fn configured_editor_mode() -> EditorMode {
    configured_tui_settings().editor_mode
}

pub fn load_editor_mode_from_home(home: &Path) -> EditorMode {
    load_tui_settings_from_home(home).editor_mode
}

pub fn configured_theme() -> Theme {
    configured_tui_settings().theme
}

pub fn load_theme_from_home(home: &Path) -> Theme {
    load_tui_settings_from_home(home).theme
}
