use super::{
    MoveAnalyzerAdapterEnvironment, MoveAnalyzerAdapterSource, MoveAnalyzerAdapterSourceStatus,
};
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn status(
    environment: &MoveAnalyzerAdapterEnvironment,
    configured_binary_path: Option<&str>,
) -> MoveAnalyzerAdapterSourceStatus {
    binary_status(
        MoveAnalyzerAdapterSource::System,
        executable(environment, configured_binary_path),
    )
}

pub(crate) fn executable(
    environment: &MoveAnalyzerAdapterEnvironment,
    configured_binary_path: Option<&str>,
) -> Option<PathBuf> {
    if let Some(path) = configured_binary_path {
        return Some(PathBuf::from(path));
    }

    find_on_path(move_analyzer_binary_name(), environment.path.as_ref()).or_else(|| {
        environment
            .search_common_user_locations
            .then(|| find_common_user_move_analyzer_binary(move_analyzer_binary_name()))
            .flatten()
    })
}

fn binary_status(
    source: MoveAnalyzerAdapterSource,
    executable: Option<PathBuf>,
) -> MoveAnalyzerAdapterSourceStatus {
    let Some(executable) = executable else {
        return MoveAnalyzerAdapterSourceStatus {
            source,
            available: false,
            version: None,
            path: None,
            error: Some(format!("{} was not found.", source.label())),
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

            MoveAnalyzerAdapterSourceStatus {
                source,
                available: output.status.success(),
                version: parse_move_analyzer_version(version_source),
                path: Some(display_path(&executable)),
                error: (!output.status.success()).then(|| {
                    if version_source.is_empty() {
                        format!("{} did not report a version.", source.label())
                    } else {
                        version_source.to_string()
                    }
                }),
            }
        }
        Err(error) => MoveAnalyzerAdapterSourceStatus {
            source,
            available: false,
            version: None,
            path: Some(display_path(&executable)),
            error: Some(format!("Could not run {}: {error}", source.label())),
        },
    }
}

fn parse_move_analyzer_version(source: &str) -> Option<String> {
    source
        .split_whitespace()
        .map(|token| token.trim_start_matches('v'))
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

fn find_common_user_move_analyzer_binary(binary_name: &str) -> Option<PathBuf> {
    common_user_move_analyzer_candidates(binary_name)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn common_user_move_analyzer_candidates(binary_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(home) = env::var_os(home_env_key()) {
        candidates.push(
            PathBuf::from(&home)
                .join(".sui")
                .join("bin")
                .join(binary_name),
        );
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

fn move_analyzer_binary_name() -> &'static str {
    if cfg!(windows) {
        "move-analyzer.exe"
    } else {
        "move-analyzer"
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::parse_move_analyzer_version;

    #[test]
    fn parses_first_numeric_version_token() {
        assert_eq!(
            parse_move_analyzer_version("move-analyzer 1.2.3"),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            parse_move_analyzer_version("move-analyzer v2.0.0"),
            Some("2.0.0".to_string())
        );
    }
}
