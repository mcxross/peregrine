use super::{normalized_path_string, relative_path_label};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerEntry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) depth: usize,
    pub(crate) expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExplorerAction {
    OpenFile(PathBuf),
    ToggledDirectory,
    None,
}

pub struct Explorer {
    pub(crate) root: PathBuf,
    pub(crate) expanded: BTreeSet<PathBuf>,
    pub(crate) visible: Vec<ExplorerEntry>,
    pub(crate) selected: usize,
}

impl Explorer {
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let mut expanded = BTreeSet::new();
        expanded.insert(root.clone());
        let mut explorer = Self {
            root,
            expanded,
            visible: Vec::new(),
            selected: 0,
        };
        explorer.refresh();
        Ok(explorer)
    }

    pub fn visible_entries(&self) -> &[ExplorerEntry] {
        &self.visible
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.visible
            .get(self.selected)
            .map(|entry| entry.path.as_path())
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.visible.len() {
            self.selected += 1;
        }
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub(crate) fn activate_selected(&mut self) -> ExplorerAction {
        let Some(entry) = self.visible.get(self.selected).cloned() else {
            return ExplorerAction::None;
        };
        if entry.is_dir {
            if self.expanded.contains(&entry.path) {
                self.expanded.remove(&entry.path);
            } else {
                self.expanded.insert(entry.path);
            }
            self.refresh();
            ExplorerAction::ToggledDirectory
        } else {
            ExplorerAction::OpenFile(entry.path)
        }
    }

    fn refresh(&mut self) {
        self.visible.clear();
        self.push_visible(self.root.clone(), 0);
        if self.visible.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.visible.len() {
            self.selected = self.visible.len() - 1;
        }
    }

    fn push_visible(&mut self, path: PathBuf, depth: usize) {
        let is_dir = path.is_dir();
        let expanded = is_dir && self.expanded.contains(&path);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| path.display().to_string());
        self.visible.push(ExplorerEntry {
            path: path.clone(),
            name,
            is_dir,
            depth,
            expanded,
        });
        if !expanded {
            return;
        }
        for child in sorted_children(&path) {
            self.push_visible(child.path, depth + 1);
        }
    }
}

pub(crate) struct ChildEntry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
}

fn sorted_children(path: &Path) -> Vec<ChildEntry> {
    let Ok(read_dir) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut children = read_dir
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            Some(ChildEntry {
                path: entry.path(),
                name,
                is_dir: file_type.is_dir(),
            })
        })
        .collect::<Vec<_>>();
    children.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.cmp(&right.name),
    });
    children
}
