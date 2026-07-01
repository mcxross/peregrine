#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use codex_utils_absolute_path::AbsolutePathBuf;
use include_dir::Dir;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::Hash;
use std::hash::Hasher;

use thiserror::Error;

const SYSTEM_SKILLS_DIR: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/assets/samples");

const SYSTEM_SKILLS_DIR_NAME: &str = ".system";
const SKILLS_DIR_NAME: &str = "skills";
const SYSTEM_SKILLS_MARKER_FILENAME: &str = ".peregrine-system-skills.marker";
const SYSTEM_SKILLS_MARKER_SALT: &str = "v1";

/// Returns the on-disk cache location for embedded system skills from an absolute PEREGRINE_HOME.
pub fn system_cache_root_dir(peregrine_home: &AbsolutePathBuf) -> AbsolutePathBuf {
    peregrine_home
        .join(SKILLS_DIR_NAME)
        .join(SYSTEM_SKILLS_DIR_NAME)
}

/// Installs embedded system skills into `PEREGRINE_HOME/skills/.system`.
///
/// Clears any existing system skills directory first and then writes the embedded
/// skills directory into place.
///
/// To avoid doing unnecessary work on every startup, a marker file is written
/// with a fingerprint of the embedded directory. When the marker matches, the
/// install is skipped.
pub fn install_system_skills(peregrine_home: &AbsolutePathBuf) -> Result<(), SystemSkillsError> {
    let skills_root_dir = peregrine_home.join(SKILLS_DIR_NAME);
    fs::create_dir_all(skills_root_dir.as_path())
        .map_err(|source| SystemSkillsError::io("create skills root dir", source))?;

    let dest_system = system_cache_root_dir(peregrine_home);

    let marker_path = dest_system.join(SYSTEM_SKILLS_MARKER_FILENAME);
    let expected_fingerprint = embedded_system_skills_fingerprint();
    if dest_system.as_path().is_dir()
        && read_marker(&marker_path).is_ok_and(|marker| marker == expected_fingerprint)
    {
        return Ok(());
    }

    if dest_system.as_path().exists() {
        fs::remove_dir_all(dest_system.as_path())
            .map_err(|source| SystemSkillsError::io("remove existing system skills dir", source))?;
    }

    write_embedded_dir(&SYSTEM_SKILLS_DIR, &dest_system)?;
    fs::write(marker_path.as_path(), format!("{expected_fingerprint}\n"))
        .map_err(|source| SystemSkillsError::io("write system skills marker", source))?;
    Ok(())
}

fn read_marker(path: &AbsolutePathBuf) -> Result<String, SystemSkillsError> {
    Ok(fs::read_to_string(path.as_path())
        .map_err(|source| SystemSkillsError::io("read system skills marker", source))?
        .trim()
        .to_string())
}

fn embedded_system_skills_fingerprint() -> String {
    let mut items = Vec::new();
    collect_fingerprint_items(&SYSTEM_SKILLS_DIR, &mut items);
    items.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let mut hasher = DefaultHasher::new();
    SYSTEM_SKILLS_MARKER_SALT.hash(&mut hasher);
    for (path, contents_hash) in items {
        path.hash(&mut hasher);
        contents_hash.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

fn collect_fingerprint_items(dir: &Dir<'_>, items: &mut Vec<(String, Option<u64>)>) {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(subdir) => {
                items.push((subdir.path().to_string_lossy().to_string(), None));
                collect_fingerprint_items(subdir, items);
            }
            include_dir::DirEntry::File(file) => {
                let mut file_hasher = DefaultHasher::new();
                file.contents().hash(&mut file_hasher);
                items.push((
                    file.path().to_string_lossy().to_string(),
                    Some(file_hasher.finish()),
                ));
            }
        }
    }
}

/// Writes the embedded `include_dir::Dir` to disk under `dest`.
///
/// Preserves the embedded directory structure.
fn write_embedded_dir(dir: &Dir<'_>, dest: &AbsolutePathBuf) -> Result<(), SystemSkillsError> {
    fs::create_dir_all(dest.as_path())
        .map_err(|source| SystemSkillsError::io("create system skills dir", source))?;

    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(subdir) => {
                let subdir_dest = dest.join(subdir.path());
                fs::create_dir_all(subdir_dest.as_path()).map_err(|source| {
                    SystemSkillsError::io("create system skills subdir", source)
                })?;
                write_embedded_dir(subdir, dest)?;
            }
            include_dir::DirEntry::File(file) => {
                let path = dest.join(file.path());
                if let Some(parent) = path.as_path().parent() {
                    fs::create_dir_all(parent).map_err(|source| {
                        SystemSkillsError::io("create system skills file parent", source)
                    })?;
                }
                fs::write(path.as_path(), file.contents())
                    .map_err(|source| SystemSkillsError::io("write system skill file", source))?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum SystemSkillsError {
    #[error("io error while {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl SystemSkillsError {
    fn io(action: &'static str, source: std::io::Error) -> Self {
        Self::Io { action, source }
    }
}

#[cfg(test)]
mod tests {
    use super::SYSTEM_SKILLS_DIR;
    use super::collect_fingerprint_items;
    use super::install_system_skills;
    use super::system_cache_root_dir;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::fs;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    #[test]
    fn bundled_skills_are_limited_to_peregrine_defaults() {
        let mut items = Vec::new();
        collect_fingerprint_items(&SYSTEM_SKILLS_DIR, &mut items);
        let mut paths: Vec<String> = items.into_iter().map(|(path, _)| path).collect();
        paths.sort_unstable();

        let expected_skill_docs = [
            "peregrine-move-audit/SKILL.md",
            "peregrine-security-audit/SKILL.md",
            "skill-creator/SKILL.md",
            "skill-installer/SKILL.md",
            "sui-prover/SKILL.md",
        ];

        for expected in expected_skill_docs {
            assert!(
                paths
                    .binary_search_by(|probe| probe.as_str().cmp(expected))
                    .is_ok(),
                "missing bundled skill file {expected}"
            );
        }

        assert!(
            paths.iter().all(|path| !path.starts_with("imagegen/")
                && !path.starts_with("openai-docs/")
                && !path.starts_with("plugin-creator/")),
            "unexpected upstream system skill included"
        );
    }

    #[test]
    fn install_writes_only_peregrine_system_bundle_under_given_home() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let home_path = std::env::temp_dir().join(format!(
            "peregrine-skills-test-{}-{nonce}",
            std::process::id()
        ));
        let home =
            AbsolutePathBuf::from_absolute_path_checked(&home_path).expect("absolute temp dir");

        install_system_skills(&home).expect("install system skills");

        let system_dir = system_cache_root_dir(&home);
        let mut installed: Vec<String> = fs::read_dir(system_dir.as_path())
            .expect("read system skills dir")
            .map(|entry| {
                entry
                    .expect("system skill entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .filter(|name| name != ".peregrine-system-skills.marker")
            .collect();
        installed.sort_unstable();

        assert_eq!(
            installed,
            [
                "peregrine-move-audit",
                "peregrine-security-audit",
                "skill-creator",
                "skill-installer",
                "sui-prover",
            ]
        );

        let _ = fs::remove_dir_all(home.as_path());
    }
}
