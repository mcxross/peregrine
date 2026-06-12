use super::EditorBuffer;
use crate::workbench::relative_path_label;
use std::collections::HashMap;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

const FILE_TAB_MIN_WIDTH: u16 = 10;
const FILE_TAB_MAX_WIDTH: u16 = 28;
pub(crate) const FILE_TAB_CONTROL_WIDTH: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DocumentId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DocumentInteractionState {
    pub(crate) standard_editing: bool,
    pub(crate) vim_state: super::super::VimState,
}

impl Default for DocumentInteractionState {
    fn default() -> Self {
        Self {
            standard_editing: false,
            vim_state: super::super::VimState::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DocumentActivation {
    pub(crate) interaction: DocumentInteractionState,
    pub(crate) opened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DocumentCloseResult {
    pub(crate) was_active: bool,
    pub(crate) interaction: DocumentInteractionState,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VisibleFileTab<'a> {
    pub(crate) id: DocumentId,
    pub(crate) label: &'a str,
    pub(crate) dirty: bool,
    pub(crate) active: bool,
    pub(crate) width: u16,
}

struct OpenDocument {
    id: DocumentId,
    canonical_path: PathBuf,
    label: String,
    label_width: u16,
    buffer: EditorBuffer,
    interaction: DocumentInteractionState,
}

impl OpenDocument {
    fn update_label(&mut self, label: String) {
        self.label_width = file_tab_width(&label);
        self.label = label;
    }
}

pub(crate) struct EditorWorkspace {
    root: PathBuf,
    documents: Vec<OpenDocument>,
    path_index: HashMap<PathBuf, DocumentId>,
    id_index: HashMap<DocumentId, usize>,
    filename_index: HashMap<String, Vec<DocumentId>>,
    active: Option<usize>,
    first_visible: usize,
    next_id: u64,
    empty: EditorBuffer,
}

impl EditorWorkspace {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
            documents: Vec::new(),
            path_index: HashMap::new(),
            id_index: HashMap::new(),
            filename_index: HashMap::new(),
            active: None,
            first_visible: 0,
            next_id: 0,
            empty: EditorBuffer::new_empty(),
        }
    }

    pub(crate) fn open_file(
        &mut self,
        path: &Path,
        current_interaction: DocumentInteractionState,
    ) -> io::Result<DocumentActivation> {
        let canonical_path = path.canonicalize()?;
        if let Some(id) = self.path_index.get(&canonical_path).copied() {
            let interaction = self.activate(id, current_interaction);
            return Ok(DocumentActivation {
                interaction,
                opened: false,
            });
        }
        if self.documents.is_empty() && self.empty.dirty {
            return Err(io::Error::other(
                "unsaved changes exist in the empty editor buffer",
            ));
        }

        self.store_active_interaction(current_interaction);
        let mut buffer = EditorBuffer::new_empty();
        buffer.open_file(&canonical_path)?;
        let id = DocumentId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let filename = filename_label(&canonical_path);
        let index = self.documents.len();
        self.documents.push(OpenDocument {
            id,
            canonical_path: canonical_path.clone(),
            label: filename.clone(),
            label_width: file_tab_width(&filename),
            buffer,
            interaction: DocumentInteractionState::default(),
        });
        self.path_index.insert(canonical_path, id);
        self.id_index.insert(id, index);
        self.filename_index
            .entry(filename.clone())
            .or_default()
            .push(id);
        self.update_duplicate_labels(&filename);
        self.active = Some(index);

        Ok(DocumentActivation {
            interaction: DocumentInteractionState::default(),
            opened: true,
        })
    }

    pub(crate) fn activate(
        &mut self,
        id: DocumentId,
        current_interaction: DocumentInteractionState,
    ) -> DocumentInteractionState {
        self.store_active_interaction(current_interaction);
        let Some(index) = self.id_index.get(&id).copied() else {
            return current_interaction;
        };
        self.active = Some(index);
        self.documents[index].interaction
    }

    pub(crate) fn select_previous(
        &mut self,
        current_interaction: DocumentInteractionState,
    ) -> DocumentInteractionState {
        let Some(active) = self.active else {
            return current_interaction;
        };
        let target = if active == 0 {
            self.documents.len() - 1
        } else {
            active - 1
        };
        let id = self.documents[target].id;
        self.activate(id, current_interaction)
    }

    pub(crate) fn select_next(
        &mut self,
        current_interaction: DocumentInteractionState,
    ) -> DocumentInteractionState {
        let Some(active) = self.active else {
            return current_interaction;
        };
        let target = (active + 1) % self.documents.len();
        let id = self.documents[target].id;
        self.activate(id, current_interaction)
    }

    pub(crate) fn select_first(
        &mut self,
        current_interaction: DocumentInteractionState,
    ) -> DocumentInteractionState {
        let Some(document) = self.documents.first() else {
            return current_interaction;
        };
        self.activate(document.id, current_interaction)
    }

    pub(crate) fn select_last(
        &mut self,
        current_interaction: DocumentInteractionState,
    ) -> DocumentInteractionState {
        let Some(document) = self.documents.last() else {
            return current_interaction;
        };
        self.activate(document.id, current_interaction)
    }

    pub(crate) fn close(
        &mut self,
        id: DocumentId,
        current_interaction: DocumentInteractionState,
    ) -> Option<DocumentCloseResult> {
        let index = self.id_index.get(&id).copied()?;
        let was_active = self.active == Some(index);
        if was_active {
            self.store_active_interaction(current_interaction);
        }
        let document = self.documents.remove(index);
        self.path_index.remove(&document.canonical_path);
        self.id_index.remove(&id);
        let filename = filename_label(&document.canonical_path);
        if let Some(ids) = self.filename_index.get_mut(&filename) {
            ids.retain(|candidate| *candidate != id);
            if ids.is_empty() {
                self.filename_index.remove(&filename);
            }
        }
        for (new_index, document) in self.documents.iter().enumerate().skip(index) {
            self.id_index.insert(document.id, new_index);
        }
        self.update_duplicate_labels(&filename);

        if self.documents.is_empty() {
            self.active = None;
            self.first_visible = 0;
            self.empty = EditorBuffer::new_empty();
            return Some(DocumentCloseResult {
                was_active,
                interaction: DocumentInteractionState::default(),
            });
        }

        if was_active {
            self.active = Some(index.min(self.documents.len() - 1));
        } else if self.active.is_some_and(|active| active > index) {
            self.active = self.active.map(|active| active - 1);
        }
        self.first_visible = self.first_visible.min(self.documents.len() - 1);
        let interaction = self
            .active
            .map(|active| self.documents[active].interaction)
            .unwrap_or_default();
        Some(DocumentCloseResult {
            was_active,
            interaction,
        })
    }

    pub(crate) fn save_document(&mut self, id: DocumentId) -> io::Result<()> {
        let Some(index) = self.id_index.get(&id).copied() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "open document not found",
            ));
        };
        self.documents[index].buffer.save()
    }

    pub(crate) fn document_is_dirty(&self, id: DocumentId) -> bool {
        self.id_index
            .get(&id)
            .is_some_and(|index| self.documents[*index].buffer.dirty)
    }

    pub(crate) fn document_label(&self, id: DocumentId) -> Option<&str> {
        let index = self.id_index.get(&id)?;
        Some(&self.documents[*index].label)
    }

    pub(crate) fn active_id(&self) -> Option<DocumentId> {
        self.active.map(|index| self.documents[index].id)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.documents.len()
    }

    pub(crate) fn ensure_active_visible(&mut self, width: u16) {
        let Some(active) = self.active else {
            self.first_visible = 0;
            return;
        };
        if active < self.first_visible {
            self.first_visible = active;
        }
        let (_, end) = self.visible_range(width);
        if active < end {
            return;
        }

        self.first_visible = active;
        let right_control = if active + 1 < self.documents.len() {
            FILE_TAB_CONTROL_WIDTH
        } else {
            0
        };
        let mut used = self.documents[active].label_width;
        while self.first_visible > 0 {
            let previous = self.first_visible - 1;
            let next_used = used.saturating_add(self.documents[previous].label_width);
            let left_control = if previous > 0 {
                FILE_TAB_CONTROL_WIDTH
            } else {
                0
            };
            if next_used
                .saturating_add(left_control)
                .saturating_add(right_control)
                > width
            {
                break;
            }
            self.first_visible = previous;
            used = next_used;
        }
    }

    pub(crate) fn visible_tabs(&self, width: u16) -> Vec<VisibleFileTab<'_>> {
        let (start, end) = self.visible_range(width);
        self.documents[start..end]
            .iter()
            .enumerate()
            .map(|(offset, document)| VisibleFileTab {
                id: document.id,
                label: &document.label,
                dirty: document.buffer.dirty,
                active: self.active == Some(start + offset),
                width: document.label_width,
            })
            .collect()
    }

    pub(crate) fn has_hidden_before(&self) -> bool {
        self.first_visible > 0
    }

    pub(crate) fn has_hidden_after(&self, width: u16) -> bool {
        self.visible_range(width).1 < self.documents.len()
    }

    pub(crate) fn page_left(&mut self, width: u16) {
        if self.first_visible == 0 {
            return;
        }
        let current = self.first_visible;
        self.first_visible = current - 1;
        while self.first_visible > 0 {
            let (start, end) = self.visible_range(width);
            if end >= current {
                break;
            }
            self.first_visible = start - 1;
        }
    }

    pub(crate) fn page_right(&mut self, width: u16) {
        let (_, end) = self.visible_range(width);
        if end < self.documents.len() {
            self.first_visible = end;
        }
    }

    fn visible_range(&self, width: u16) -> (usize, usize) {
        if self.documents.is_empty() || width == 0 {
            return (0, 0);
        }
        let start = self.first_visible.min(self.documents.len() - 1);
        let left_width = if start > 0 { FILE_TAB_CONTROL_WIDTH } else { 0 };
        let without_right = width.saturating_sub(left_width);
        let end = visible_end(&self.documents, start, without_right);
        if end == self.documents.len() {
            return (start, end);
        }
        let with_right = without_right.saturating_sub(FILE_TAB_CONTROL_WIDTH);
        (start, visible_end(&self.documents, start, with_right))
    }

    fn store_active_interaction(&mut self, interaction: DocumentInteractionState) {
        if let Some(active) = self.active {
            self.documents[active].interaction = interaction;
        }
    }

    fn update_duplicate_labels(&mut self, filename: &str) {
        let Some(ids) = self.filename_index.get(filename).cloned() else {
            return;
        };
        let duplicate = ids.len() > 1;
        for id in ids {
            let Some(index) = self.id_index.get(&id).copied() else {
                continue;
            };
            let label = if duplicate {
                relative_path_label(&self.root, &self.documents[index].canonical_path)
            } else {
                filename.to_string()
            };
            self.documents[index].update_label(label);
        }
    }
}

impl Deref for EditorWorkspace {
    type Target = EditorBuffer;

    fn deref(&self) -> &Self::Target {
        self.active
            .map(|index| &self.documents[index].buffer)
            .unwrap_or(&self.empty)
    }
}

impl DerefMut for EditorWorkspace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.active
            .map(|index| &mut self.documents[index].buffer)
            .unwrap_or(&mut self.empty)
    }
}

fn visible_end(documents: &[OpenDocument], start: usize, width: u16) -> usize {
    let mut used = 0_u16;
    let mut end = start;
    while let Some(document) = documents.get(end) {
        if used.saturating_add(document.label_width) > width {
            break;
        }
        used = used.saturating_add(document.label_width);
        end += 1;
    }
    if end == start && width > 0 {
        (start + 1).min(documents.len())
    } else {
        end
    }
}

fn filename_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn file_tab_width(label: &str) -> u16 {
    let content = UnicodeWidthStr::width(label).saturating_add(7);
    u16::try_from(content)
        .unwrap_or(u16::MAX)
        .clamp(FILE_TAB_MIN_WIDTH, FILE_TAB_MAX_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn opening_existing_path_activates_without_duplicate() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = temp.path().join("first.move");
        let second = temp.path().join("second.move");
        fs::write(&first, "first").expect("write first");
        fs::write(&second, "second").expect("write second");
        let mut workspace = EditorWorkspace::new(temp.path());

        workspace
            .open_file(&first, DocumentInteractionState::default())
            .expect("open first");
        workspace.insert_char('!');
        workspace
            .open_file(&second, DocumentInteractionState::default())
            .expect("open second");
        let activation = workspace
            .open_file(&first, DocumentInteractionState::default())
            .expect("activate first");

        assert_eq!(workspace.len(), 2);
        assert!(!activation.opened);
        assert_eq!(workspace.text(), "!first");
    }

    #[test]
    fn duplicate_filenames_use_relative_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = temp.path().join("a/main.move");
        let second = temp.path().join("b/main.move");
        fs::create_dir_all(first.parent().expect("first parent")).expect("create first");
        fs::create_dir_all(second.parent().expect("second parent")).expect("create second");
        fs::write(&first, "").expect("write first");
        fs::write(&second, "").expect("write second");
        let mut workspace = EditorWorkspace::new(temp.path());

        workspace
            .open_file(&first, DocumentInteractionState::default())
            .expect("open first");
        workspace
            .open_file(&second, DocumentInteractionState::default())
            .expect("open second");
        let labels = workspace
            .documents
            .iter()
            .map(|document| document.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(labels, vec!["a/main.move", "b/main.move"]);
    }

    #[test]
    fn switching_documents_restores_buffer_and_interaction_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = temp.path().join("first.move");
        let second = temp.path().join("second.move");
        fs::write(&first, "first").expect("write first");
        fs::write(&second, "second").expect("write second");
        let mut workspace = EditorWorkspace::new(temp.path());
        workspace
            .open_file(&first, DocumentInteractionState::default())
            .expect("open first");
        workspace.cursor.col = 3;
        workspace.horizontal_scroll = 2;
        workspace.insert_char('!');
        workspace
            .open_file(
                &second,
                DocumentInteractionState {
                    standard_editing: false,
                    vim_state: super::super::super::VimState::Insert,
                },
            )
            .expect("open second");

        let activation = workspace
            .open_file(&first, DocumentInteractionState::default())
            .expect("activate first");

        assert_eq!(
            activation.interaction,
            DocumentInteractionState {
                standard_editing: false,
                vim_state: super::super::super::VimState::Insert,
            }
        );
        assert_eq!(workspace.cursor.col, 4);
        assert_eq!(workspace.horizontal_scroll, 4);
        assert!(workspace.dirty);
        assert_eq!(workspace.text(), "fir!st");
    }

    #[test]
    fn visible_window_stays_bounded_with_thousands_of_documents() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut workspace = EditorWorkspace::new(temp.path());
        for index in 0..2_000 {
            let id = DocumentId(index);
            let label = format!("file-{index:04}.move");
            let path = temp.path().join(&label);
            workspace.documents.push(OpenDocument {
                id,
                canonical_path: path.clone(),
                label: label.clone(),
                label_width: file_tab_width(&label),
                buffer: EditorBuffer::new_empty(),
                interaction: DocumentInteractionState::default(),
            });
            workspace.path_index.insert(path, id);
            workspace.id_index.insert(id, index as usize);
        }
        workspace.active = Some(1_999);

        workspace.ensure_active_visible(80);
        let visible = workspace.visible_tabs(80);

        assert!(visible.len() < 10);
        assert!(visible.iter().any(|tab| tab.active));
        assert!(workspace.has_hidden_before());
    }

    #[test]
    fn active_tab_remains_visible_when_both_overflow_controls_are_needed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut workspace = EditorWorkspace::new(temp.path());
        for index in 0..8 {
            let id = DocumentId(index);
            let label = format!("file-{index}.move");
            workspace.documents.push(OpenDocument {
                id,
                canonical_path: temp.path().join(&label),
                label,
                label_width: 14,
                buffer: EditorBuffer::new_empty(),
                interaction: DocumentInteractionState::default(),
            });
            workspace.id_index.insert(id, index as usize);
        }
        workspace.active = Some(5);

        workspace.ensure_active_visible(30);

        assert!(workspace.visible_tabs(30).iter().any(|tab| tab.active));
        assert!(workspace.has_hidden_before());
        assert!(workspace.has_hidden_after(30));
    }
}
