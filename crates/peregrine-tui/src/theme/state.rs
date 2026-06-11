use super::{Theme, ThemeName, ThemePalette};
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Debug, Clone)]
pub struct ThemeState {
    inner: Arc<RwLock<ThemeStateInner>>,
}

#[derive(Debug, Clone, Copy)]
struct ThemeStateInner {
    theme: Theme,
    generation: u64,
}

impl ThemeState {
    #[must_use]
    pub fn new(theme: Theme) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ThemeStateInner {
                theme,
                generation: 0,
            })),
        }
    }

    #[must_use]
    pub fn current(&self) -> Theme {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .theme
    }

    #[must_use]
    pub fn current_name(&self) -> ThemeName {
        self.current().name
    }

    #[must_use]
    pub fn palette(&self) -> ThemePalette {
        self.current().palette()
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation
    }

    pub fn set(&self, name: ThemeName) {
        let mut state = self
            .inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.theme.name != name {
            state.theme = Theme::new(name);
            state.generation = state.generation.wrapping_add(1);
        }
    }

    pub fn next(&self) {
        self.set(self.current_name().next());
    }

    pub fn previous(&self) {
        self.set(self.current_name().prev());
    }
}

impl Default for ThemeState {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

static SHARED_THEME_STATE: OnceLock<ThemeState> = OnceLock::new();

#[must_use]
pub fn shared_theme_state() -> ThemeState {
    SHARED_THEME_STATE.get_or_init(ThemeState::default).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_observe_theme_changes_and_generation() {
        let state = ThemeState::default();
        let observer = state.clone();

        state.set(ThemeName::ZeroDay);

        assert_eq!(observer.current_name(), ThemeName::ZeroDay);
        assert_eq!(observer.generation(), 1);
    }
}
