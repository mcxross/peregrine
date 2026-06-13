use super::{
    SuiAdapterEnvironment, SuiAdapterError, SuiAdapterSource, SuiAdapterSourceStatus,
    SuiCommandOutput, SuiMoveBuildOptions,
};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn status(
    environment: &SuiAdapterEnvironment,
    configured_cli_path: Option<&str>,
) -> SuiAdapterSourceStatus {
    binary_status(
        SuiAdapterSource::System,
        executable(environment, configured_cli_path),
    )
}

pub(crate) fn executable(
    environment: &SuiAdapterEnvironment,
    configured_cli_path: Option<&str>,
) -> Option<PathBuf> {
    if let Some(path) = configured_cli_path {
        return Some(PathBuf::from(path));
    }

    find_on_path(sui_binary_name(), environment.path.as_ref()).or_else(|| {
        environment
            .search_common_user_locations
            .then(|| find_common_user_sui_binary(sui_binary_name()))
            .flatten()
    })
}

pub(crate) fn run_blocking(
    executable: &Path,
    args: &[String],
    package_root: &Path,
) -> Result<SuiCommandOutput, SuiAdapterError> {
    let output = Command::new(executable)
        .args(args)
        .current_dir(package_root)
        .output()
        .map_err(|error| SuiAdapterError::CommandExecution(error.to_string()))?;
    Ok(SuiCommandOutput {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn run_move_build_blocking(
    executable: &Path,
    package_root: &Path,
    options: &SuiMoveBuildOptions,
) -> Result<SuiCommandOutput, SuiAdapterError> {
    let move_home = package_root.join(".move-home");
    let sui_config_dir = package_root.join(".sui-config");
    fs::create_dir_all(&move_home)
        .map_err(|error| SuiAdapterError::CommandExecution(error.to_string()))?;
    fs::create_dir_all(&sui_config_dir)
        .map_err(|error| SuiAdapterError::CommandExecution(error.to_string()))?;

    let mut command = Command::new(executable);
    command.args(["move", "build", "--path"]).arg(package_root);
    if let Some(default_move_flavor) = &options.default_move_flavor {
        command.args(["--default-move-flavor", default_move_flavor]);
    }
    let output = command
        .env("MOVE_HOME", move_home)
        .env("SUI_CONFIG_DIR", sui_config_dir)
        .output()
        .map_err(|error| SuiAdapterError::CommandExecution(error.to_string()))?;

    Ok(SuiCommandOutput {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
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
    if cfg!(windows) { "USERPROFILE" } else { "HOME" }
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
    if cfg!(windows) { "sui.exe" } else { "sui" }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
