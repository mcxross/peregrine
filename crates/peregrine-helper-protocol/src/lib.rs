use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const BYTECODE_VIEWER_HELPER_ARG: &str = "--peregrine-bytecode-viewer";
pub const BUNDLED_SUI_HELPER_ARG: &str = "--peregrine-bundled-sui";
pub const FORMAL_VERIFICATION_HELPER_ARG: &str = "--peregrine-formal-verification";
pub const HELPER_ENV_VAR: &str = "PEREGRINE_HELPER";
pub const JSON_PROTOCOL_HELPER_ARG: &str = "--peregrine-helper-json";
pub const MOVE_ANALYZER_HELPER_ARG: &str = "--peregrine-move-analyzer";
pub const MOVY_FUZZ_HELPER_ARG: &str = "--peregrine-movy-fuzz";

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum HelperRequest {
    Ping,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelperResponse {
    pub error: Option<String>,
    pub ok: bool,
    pub status: Option<i32>,
    pub stderr: String,
    pub stdout: String,
}

impl HelperResponse {
    pub fn ok(status: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            error: None,
            ok: true,
            status: Some(status),
            stderr: stderr.into(),
            stdout: stdout.into(),
        }
    }

    pub fn error(status: i32, error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ok: false,
            status: Some(status),
            stderr: String::new(),
            stdout: String::new(),
        }
    }
}

pub fn parse_helper_request(input: &[u8]) -> Result<HelperRequest, String> {
    serde_json::from_slice(input).map_err(|error| format!("Invalid helper request JSON: {error}"))
}

pub fn helper_binary_file_name() -> &'static str {
    if cfg!(windows) {
        "peregrine-helper.exe"
    } else {
        "peregrine-helper"
    }
}

pub fn resolve_external_helper_executable() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    resolve_external_helper_executable_from(&current_exe, std::env::var_os(HELPER_ENV_VAR))
}

pub fn resolve_helper_executable() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("Could not resolve Peregrine executable: {error}"))?;
    resolve_helper_executable_from(&current_exe, std::env::var_os(HELPER_ENV_VAR))
}

pub fn resolve_helper_executable_for_current_exe(current_exe: &Path) -> Result<PathBuf, String> {
    resolve_helper_executable_from(current_exe, std::env::var_os(HELPER_ENV_VAR))
}

fn resolve_helper_executable_from(
    current_exe: &Path,
    env_override: Option<OsString>,
) -> Result<PathBuf, String> {
    resolve_external_helper_executable_from(current_exe, env_override).ok_or_else(|| {
        "Peregrine helper is unavailable. Build or install peregrine-helper beside the \
         Peregrine executable, or set PEREGRINE_HELPER."
            .to_string()
    })
}

pub fn resolve_external_helper_executable_from(
    current_exe: &Path,
    env_override: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(candidate) = env_override
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| is_distinct_file(path, current_exe))
    {
        return Some(candidate);
    }

    let sibling = current_exe.with_file_name(helper_binary_file_name());

    if is_distinct_file(&sibling, current_exe) {
        Some(sibling)
    } else {
        None
    }
}

fn is_distinct_file(candidate: &Path, current_exe: &Path) -> bool {
    if !candidate.is_file() {
        return false;
    }

    match (candidate.canonicalize(), current_exe.canonicalize()) {
        (Ok(candidate), Ok(current)) => candidate != current,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn helper_path_resolution_prefers_env_override() {
        let directory = tempdir().expect("tempdir");
        let current = directory.path().join("peregrine");
        let env_helper = directory.path().join("custom-helper");
        let sibling_helper = directory.path().join(helper_binary_file_name());
        fs::write(&current, "").expect("write current");
        fs::write(&env_helper, "").expect("write env helper");
        fs::write(&sibling_helper, "").expect("write sibling helper");

        assert_eq!(
            resolve_external_helper_executable_from(&current, Some(env_helper.clone().into())),
            Some(env_helper),
        );
    }

    #[test]
    fn helper_path_resolution_uses_sibling_helper() {
        let directory = tempdir().expect("tempdir");
        let current = directory.path().join("peregrine");
        let helper = directory.path().join(helper_binary_file_name());
        fs::write(&current, "").expect("write current");
        fs::write(&helper, "").expect("write helper");

        assert_eq!(
            resolve_external_helper_executable_from(&current, None),
            Some(helper)
        );
    }

    #[test]
    fn helper_path_resolution_does_not_return_current_executable() {
        let directory = tempdir().expect("tempdir");
        let current = directory.path().join(helper_binary_file_name());
        fs::write(&current, "").expect("write current");

        assert_eq!(
            resolve_external_helper_executable_from(&current, Some(current.clone().into())),
            None,
        );
    }

    #[test]
    fn required_helper_path_resolution_does_not_fall_back_to_current_executable() {
        let directory = tempdir().expect("tempdir");
        let current = directory.path().join(helper_binary_file_name());
        fs::write(&current, "").expect("write current");

        let error = resolve_helper_executable_from(&current, Some(current.clone().into()))
            .expect_err("current executable must not be used as its own helper");

        assert!(error.contains("Peregrine helper is unavailable"));
    }

    #[test]
    fn helper_request_reports_invalid_json() {
        let error = parse_helper_request(b"{not-json").expect_err("invalid json");

        assert!(error.contains("Invalid helper request JSON"));
    }
}
