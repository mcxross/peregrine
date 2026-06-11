use crate::agent::legacy_core::config::{Config, ConfigBuilder, ConfigOverrides};
use crate::session::app_server::AppServerSession;
use crate::theme::{Theme, ThemeName, ThemeState, shared_theme_state};
use crate::{EditorMode, build_agent_runtime};
use peregrine_config::LoaderOverrides;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UiRuntimeConfig {
    pub(crate) editor_mode: EditorMode,
    pub(crate) theme: Theme,
}

impl UiRuntimeConfig {
    fn from_config(config: &Config) -> Self {
        let editor_mode = if config.tui_vim_mode_default {
            EditorMode::Vim
        } else {
            EditorMode::Standard
        };
        let theme = config
            .tui_theme
            .as_deref()
            .and_then(|name| name.parse::<ThemeName>().ok())
            .map(Theme::new)
            .unwrap_or_default();
        Self { editor_mode, theme }
    }
}

#[derive(Clone)]
pub(crate) struct ApplicationRuntime {
    config: Arc<Config>,
    ui: UiRuntimeConfig,
    theme: ThemeState,
    app_server: Arc<Mutex<Option<AppServerSession>>>,
}

impl ApplicationRuntime {
    pub(crate) fn load(root: PathBuf) -> io::Result<Self> {
        let config = build_config(root, None)?;
        Ok(Self::from_config(config))
    }

    pub(crate) fn load_from_home(root: PathBuf, peregrine_home: PathBuf) -> io::Result<Self> {
        let config = build_config(root, Some(peregrine_home))?;
        Ok(Self::from_config(config))
    }

    fn from_config(config: Config) -> Self {
        let ui = UiRuntimeConfig::from_config(&config);
        let theme = shared_theme_state();
        theme.set(ui.theme.name);
        Self {
            config: Arc::new(config),
            ui,
            theme,
            app_server: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn config(&self) -> Arc<Config> {
        self.config.clone()
    }

    pub(crate) fn ui(&self) -> UiRuntimeConfig {
        self.ui
    }

    pub(crate) fn theme(&self) -> ThemeState {
        self.theme.clone()
    }

    pub(crate) fn take_app_server(&self) -> Option<AppServerSession> {
        self.app_server
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub(crate) fn store_app_server(&self, app_server: AppServerSession) {
        let previous = self
            .app_server
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(app_server);
        debug_assert!(
            previous.is_none(),
            "application runtime already owns an app-server session"
        );
    }
}

fn build_config(root: PathBuf, peregrine_home: Option<PathBuf>) -> io::Result<Config> {
    let mut builder = ConfigBuilder::default()
        .harness_overrides(ConfigOverrides {
            cwd: Some(root),
            peregrine_self_exe: std::env::current_exe().ok(),
            ..ConfigOverrides::default()
        })
        .loader_overrides(LoaderOverrides::default());
    if let Some(peregrine_home) = peregrine_home {
        builder = builder.peregrine_home(peregrine_home);
    }
    build_agent_runtime()?.block_on(builder.build())
}
