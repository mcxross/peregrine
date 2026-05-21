use std::{fs, path::Path};

pub(crate) fn collect_project_paths(root: &Path) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    collect_paths(root, root, &mut paths)?;
    paths.sort_by(|left, right| compare_tree_paths(left, right));
    Ok(paths)
}

fn collect_paths(root: &Path, directory: &Path, paths: &mut Vec<String>) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if directory == root => {
            return Err(format!(
                "Could not read package directory {}: {error}",
                directory.display()
            ));
        }
        Err(_) => return Ok(()),
    };

    let mut entries = entries
        .filter_map(Result::ok)
        .collect::<Vec<fs::DirEntry>>();
    entries.sort_by(compare_dir_entries);

    for entry in entries {
        let path = entry.path();
        let Ok(relative_path) = path.strip_prefix(root) else {
            continue;
        };
        let Some(relative_path) = normalize_tree_path(relative_path) else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            paths.push(relative_path);
            continue;
        };

        if file_type.is_dir() {
            if should_skip_tree_directory(&path) {
                continue;
            }

            paths.push(format!("{relative_path}/"));
            collect_paths(root, &path, paths)?;
        } else {
            paths.push(relative_path);
        }
    }

    Ok(())
}

fn should_skip_tree_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    matches!(
        name,
        ".git"
            | ".next"
            | ".peregrine"
            | ".sui"
            | ".turbo"
            | "build"
            | "coverage"
            | "dist"
            | "node_modules"
            | "package_summaries"
            | "target"
    )
}

fn compare_dir_entries(left: &fs::DirEntry, right: &fs::DirEntry) -> std::cmp::Ordering {
    let left_is_dir = left
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or(false);
    let right_is_dir = right
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or(false);

    right_is_dir
        .cmp(&left_is_dir)
        .then_with(|| left.file_name().cmp(&right.file_name()))
}

fn compare_tree_paths(left: &str, right: &str) -> std::cmp::Ordering {
    let left_is_dir = left.ends_with('/');
    let right_is_dir = right.ends_with('/');

    right_is_dir.cmp(&left_is_dir).then_with(|| left.cmp(right))
}

fn normalize_tree_path(path: &Path) -> Option<String> {
    Some(
        path.components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}
