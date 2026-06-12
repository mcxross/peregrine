use crate::theme::ThemeName;
use crate::workbench_render::RenderedWorkbenchDocument;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub(crate) row: usize,
    pub(crate) col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSnapshot {
    pub(crate) lines: Vec<String>,
    pub(crate) cursor: Cursor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorRenderCache {
    pub(crate) path: Option<PathBuf>,
    pub(crate) source: String,
    pub(crate) theme: ThemeName,
    pub(crate) markdown_preview: bool,
    pub(crate) width: usize,
    pub(crate) root: PathBuf,
    pub(crate) document: RenderedWorkbenchDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingVimCommand {
    Delete,
    Yank,
}
