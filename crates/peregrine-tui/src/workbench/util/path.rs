use std::path::{Path, PathBuf};

pub(crate) fn nearest_move_package_root(path: &Path, project_root: &Path) -> Option<PathBuf> {
    let project_root = project_root.canonicalize().ok()?;
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    }
    .canonicalize()
    .ok()?;

    loop {
        if current.join("Move.toml").is_file() {
            return Some(current);
        }
        if current == project_root || !current.pop() {
            return None;
        }
        if !current.starts_with(&project_root) {
            return None;
        }
    }
}

pub(crate) fn relative_path_label(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);

    if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        normalized_path_string(relative)
    }
}

pub(crate) fn normalized_path_string(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            std::path::Component::CurDir => None,
            other => Some(other.as_os_str().to_string_lossy().into_owned()),
        })
        .collect::<Vec<_>>()
        .join("/")
}
