mod prelude;

mod app;
mod app_drop;
mod bytecode;
mod command_input;
mod constants;
mod editor;
mod events;
mod explorer;
mod graphs;
mod render;
mod settings;
mod startup;
mod status;
mod types;
mod util;

#[allow(dead_code)]
pub(crate) const AGENT_TOKIO_WORKER_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

pub use app::App;
pub use command_input::CommandInput;
pub use editor::EditorBuffer;
pub use explorer::{Explorer, ExplorerEntry};
pub use settings::{
    TuiSettings, configured_editor_mode, configured_theme, configured_tui_settings,
    load_editor_mode_from_home, load_theme_from_home, load_tui_settings_from_home,
};
pub use types::{AppMode, EditorMode, FocusPane, VimState, WorkbenchExit, WorkbenchTab, GraphTab};

pub(crate) use constants::*;
pub(crate) use editor::{
    DocumentId, DocumentInteractionState, EditorRenderCache, EditorWorkspace,
    FILE_TAB_CONTROL_WIDTH, PendingVimCommand, VisibleFileTab,
};
pub(crate) use explorer::ExplorerAction;
pub(crate) use graphs::{
    GraphDocument, GraphLoadResult, GraphPane, GraphPanes, WorkbenchGraphContext,
};
pub(crate) use types::{
    CloseChoice, CloseConfirmation, FileTabHitArea, FileTabHitTarget, InvalidPackageAction,
    InvalidPackagePrompt, PackageLoadRunningState, PackageNamePrompt, ScrollDirection,
    StartupTaskResult, TrustAction, TrustPostAction, TrustPrompt, WorkbenchLayout,
    WorkbenchStartupState,
};

pub(crate) use bytecode::{
    BytecodeCacheEntry, BytecodeCacheStamp, BytecodeLoadResult, BytecodeLoadState, BytecodeOptions,
    BytecodePane, BytecodeRequest, BytecodeSelector, BytecodeSession, BytecodeTargetKey,
};
pub(crate) use startup::{
    default_package_name, is_quit_key, package_name_error, render_cli_step_summary,
    startup_failure_load_report, startup_option_line,
};
pub(crate) use status::focused_title;
pub(crate) use status::{
    format_elapsed, package_load_spinner, package_load_status, package_load_status_spans,
};
pub(crate) use util::{
    bytecode_cache_stamp, centered_rect, char_len, char_to_byte_index, editable_char_modifiers,
    inner_rect, nearest_move_package_root, normalized_path_string, rect_contains,
    relative_path_label, split_lines, styled_text_segments, usize_to_u16_saturating,
};

#[cfg(test)]
pub(crate) use graphs::text::{filter_type_graph, render_type_graph_text};

#[cfg(test)]
mod tests;
