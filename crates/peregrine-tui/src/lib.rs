#![allow(
    clippy::filter_next,
    clippy::module_inception,
    clippy::explicit_counter_loop,
    clippy::needless_return,
    clippy::uninlined_format_args,
    clippy::derivable_impls,
    clippy::large_enum_variant,
    clippy::result_large_err
)]
#![allow(
    clippy::unnecessary_to_owned,
    clippy::too_many_arguments,
    clippy::collapsible_if
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_imports,
    clippy::match_like_matches_macro
)]
#[path = "app/agent_ui/mod.rs"]
mod agent;
mod app;
mod args;
pub(crate) mod bootstrap;
mod chat;
pub mod helper_args;
mod keybinds;
mod navigation;
mod output;
mod session;
pub mod sui;
pub mod tabs;
pub mod theme;
mod workbench;
mod workbench_render;
mod workflow;

pub(crate) use bootstrap::agent::agent_arg0_dispatch_paths;
pub use bootstrap::{
    build_agent_runtime, run, run_cli_from_args, run_cli_or_helper_from_args, run_from_env_args,
    run_security_cli, run_tui,
};
pub(crate) use workbench::ScrollDirection;
pub use workbench::{
    App, AppMode, CommandInput, EditorBuffer, EditorMode, Explorer, ExplorerEntry, FocusPane,
    TuiSettings, VimState, WorkbenchExit, WorkbenchTab, configured_editor_mode, configured_theme,
    configured_tui_settings, load_editor_mode_from_home, load_theme_from_home,
    load_tui_settings_from_home,
};
