use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Clone, Debug, Default)]
pub struct CompilerArtifactSet {
    pub build_root: PathBuf,
    pub bytecode_modules: Vec<PathBuf>,
    pub source_maps: Vec<PathBuf>,
    pub debug_info: Vec<PathBuf>,
    pub sources: Vec<PathBuf>,
}

impl CompilerArtifactSet {
    pub fn discover(build_root: &Path) -> Self {
        let mut set = Self {
            build_root: build_root.to_path_buf(),
            ..Self::default()
        };
        set.bytecode_modules = collect_files(&build_root.join("bytecode_modules"), &["mv"]);
        set.source_maps = collect_files(&build_root.join("source_maps"), &["mvsm", "json"]);
        set.debug_info = collect_files(&build_root.join("debug_info"), &["json", "bcs"]);
        set.sources = collect_files(&build_root.join("sources"), &["move"]);
        set
    }

    pub fn has_full_mode_inputs(&self) -> bool {
        !self.bytecode_modules.is_empty()
            || !self.source_maps.is_empty()
            || !self.sources.is_empty()
    }
}

fn collect_files(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    let mut files = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extensions.iter().any(|allowed| extension == *allowed))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}
