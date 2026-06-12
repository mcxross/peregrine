use super::super::BytecodeCacheStamp;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub(crate) fn bytecode_cache_stamp(package_root: &Path) -> BytecodeCacheStamp {
    let mut stamp = BytecodeCacheStamp::default();
    visit_bytecode_cache_files(package_root, &mut stamp);
    stamp
}

fn visit_bytecode_cache_files(path: &Path, stamp: &mut BytecodeCacheStamp) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };

    if metadata.is_file() {
        if !bytecode_cache_relevant_file(path) {
            return;
        }
        stamp.file_count = stamp.file_count.saturating_add(1);
        stamp.total_len = stamp.total_len.wrapping_add(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        stamp.latest_modified_nanos = stamp.latest_modified_nanos.max(modified);
        return;
    }

    if !metadata.is_dir() || bytecode_cache_skipped_dir(path) {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        visit_bytecode_cache_files(&entry.path(), stamp);
    }
}

fn bytecode_cache_relevant_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("move"))
        || path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| matches!(name, "Move.toml" | "Move.lock"))
}

fn bytecode_cache_skipped_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".peregrine" | ".peregrine-dev" | "build" | "target"
            )
        })
}
