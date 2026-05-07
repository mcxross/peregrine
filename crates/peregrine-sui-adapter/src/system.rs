use crate::{SuiAdapterEnvironment, SuiAdapterSource, SuiAdapterSourceStatus};
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn status(environment: &SuiAdapterEnvironment) -> SuiAdapterSourceStatus {
    binary_status(SuiAdapterSource::System, executable(environment))
}

pub(crate) fn executable(environment: &SuiAdapterEnvironment) -> Option<PathBuf> {
    find_on_path(sui_binary_name(), environment.path.as_ref()).or_else(|| {
        environment
            .search_common_user_locations
            .then(|| find_common_user_sui_binary(sui_binary_name()))
            .flatten()
    })
}

fn binary_status(source: SuiAdapterSource, executable: Option<PathBuf>) -> SuiAdapterSourceStatus {
    let Some(executable) = executable else {
        return SuiAdapterSourceStatus {
            source,
            available: false,
            version: None,
            path: None,
            error: Some(format!("{} Sui CLI was not found.", source.label())),
        };
    };

    match Command::new(&executable).arg("--version").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let version_source = stdout
                .lines()
                .chain(stderr.lines())
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("");

            SuiAdapterSourceStatus {
                source,
                available: output.status.success(),
                version: parse_sui_version(version_source),
                path: Some(display_path(&executable)),
                error: (!output.status.success()).then(|| {
                    if version_source.is_empty() {
                        format!("{} Sui CLI did not report a version.", source.label())
                    } else {
                        version_source.to_string()
                    }
                }),
            }
        }
        Err(error) => SuiAdapterSourceStatus {
            source,
            available: false,
            version: None,
            path: Some(display_path(&executable)),
            error: Some(format!("Could not run {} Sui CLI: {error}", source.label())),
        },
    }
}

fn parse_sui_version(source: &str) -> Option<String> {
    source
        .split_whitespace()
        .find(|token| {
            token
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
        .map(|version| version.trim_start_matches('v').to_string())
}

fn find_on_path(binary_name: &str, path: Option<&OsString>) -> Option<PathBuf> {
    let path = path?;

    for directory in env::split_paths(path) {
        for candidate in executable_candidates(&directory, binary_name) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn find_common_user_sui_binary(binary_name: &str) -> Option<PathBuf> {
    common_user_sui_candidates(binary_name)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn common_user_sui_candidates(binary_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(home) = env::var_os(home_env_key()) {
        candidates.push(
            PathBuf::from(home)
                .join(".cargo")
                .join("bin")
                .join(binary_name),
        );
    }

    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/opt/homebrew/bin").join(binary_name));
        candidates.push(PathBuf::from("/usr/local/bin").join(binary_name));
    }

    if cfg!(target_os = "linux") {
        candidates.push(PathBuf::from("/usr/local/bin").join(binary_name));
        candidates.push(PathBuf::from("/usr/bin").join(binary_name));
    }

    candidates
}

fn home_env_key() -> &'static str {
    if cfg!(windows) {
        "USERPROFILE"
    } else {
        "HOME"
    }
}

fn executable_candidates(directory: &Path, binary_name: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        if Path::new(binary_name).extension().is_some() {
            return vec![directory.join(binary_name)];
        }

        let extensions = env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|extension| !extension.is_empty())
                    .map(|extension| extension.trim_start_matches('.').to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["exe".to_string(), "cmd".to_string(), "bat".to_string()]);

        extensions
            .into_iter()
            .map(|extension| directory.join(format!("{binary_name}.{extension}")))
            .collect()
    }

    #[cfg(not(windows))]
    {
        vec![directory.join(binary_name)]
    }
}

fn sui_binary_name() -> &'static str {
    if cfg!(windows) {
        "sui.exe"
    } else {
        "sui"
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
