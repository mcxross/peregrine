use move_prover_boogie_backend::boogie_backend::options::BoogieFileMode;
use move_stackless_bytecode::target_filter::TargetFilterOptions;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Path, PathBuf},
    thread,
};
use sui_prover::{
    prove::{BuildConfig, DEFAULT_EXECUTION_TIMEOUT_SECONDS, GeneralConfig, execute},
    remote_config::RemoteConfig,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormalVerificationOptions {
    pub module_name: String,
    pub file_path: String,
    pub timeout_seconds: Option<usize>,
    pub verbose: bool,
    pub trace: bool,
    pub keep_temp: bool,
}

impl Default for FormalVerificationOptions {
    fn default() -> Self {
        Self {
            module_name: String::new(),
            file_path: String::new(),
            timeout_seconds: Some(DEFAULT_EXECUTION_TIMEOUT_SECONDS),
            verbose: false,
            trace: false,
            keep_temp: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormalVerificationRun {
    pub manifest: FormalVerificationManifest,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormalVerificationManifest {
    pub project_root: PathBuf,
    pub package_root: PathBuf,
    pub file_path: String,
    pub module_name: String,
    pub timeout_seconds: usize,
}

#[derive(Debug)]
pub enum FormalVerificationAdapterError {
    InvalidProjectRoot { path: PathBuf, reason: String },
    InvalidPackagePath { path: PathBuf, reason: String },
    InvalidFilePath { path: String, reason: String },
    InvalidModuleName(String),
    Join(String),
    Runtime(String),
    Verification(String),
}

impl fmt::Display for FormalVerificationAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectRoot { path, reason } => {
                write!(
                    formatter,
                    "Invalid project root {}: {reason}",
                    path.display()
                )
            }
            Self::InvalidPackagePath { path, reason } => {
                write!(
                    formatter,
                    "Invalid package path {}: {reason}",
                    path.display()
                )
            }
            Self::InvalidFilePath { path, reason } => {
                write!(
                    formatter,
                    "Invalid formal verification file `{path}`: {reason}"
                )
            }
            Self::InvalidModuleName(module_name) => {
                write!(
                    formatter,
                    "Invalid formal verification module `{module_name}`."
                )
            }
            Self::Join(reason) => write!(
                formatter,
                "Could not join formal verification task: {reason}"
            ),
            Self::Runtime(reason) => write!(
                formatter,
                "Could not start formal verification runtime: {reason}"
            ),
            Self::Verification(reason) => write!(formatter, "Formal verification failed: {reason}"),
        }
    }
}

impl std::error::Error for FormalVerificationAdapterError {}

pub fn run_formal_verification_blocking(
    project_root: impl Into<PathBuf>,
    package_path: &str,
    options: FormalVerificationOptions,
) -> Result<FormalVerificationRun, FormalVerificationAdapterError> {
    let project_root = project_root.into();
    let package_path = package_path.to_string();
    let worker = thread::Builder::new()
        .name("peregrine-formal-verification".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| FormalVerificationAdapterError::Runtime(error.to_string()))?;

            runtime.block_on(run_formal_verification(
                project_root,
                &package_path,
                options,
            ))
        })
        .map_err(|error| FormalVerificationAdapterError::Join(error.to_string()))?;

    worker
        .join()
        .map_err(|_| FormalVerificationAdapterError::Join("worker panicked".to_string()))?
}

pub async fn run_formal_verification(
    project_root: impl Into<PathBuf>,
    package_path: &str,
    options: FormalVerificationOptions,
) -> Result<FormalVerificationRun, FormalVerificationAdapterError> {
    let manifest = prepare_manifest(project_root.into(), package_path, &options)?;
    let general_config = general_config(&options);
    let filter = TargetFilterOptions {
        modules: Some(vec![manifest.module_name.clone()]),
        functions: None,
    };

    println!("Starting bundled Sui Prover formal verification.");
    println!("Package: {}", manifest.package_root.display());
    println!("File: {}", manifest.file_path);
    println!("Module filter: {}", manifest.module_name);
    println!("Timeout: {} seconds", manifest.timeout_seconds);

    execute(
        Some(manifest.package_root.as_path()),
        general_config,
        RemoteConfig::default(),
        BuildConfig::default(),
        None,
        filter,
    )
    .await
    .map_err(|error| FormalVerificationAdapterError::Verification(format!("{error:?}")))?;

    println!("Formal verification completed.");

    Ok(FormalVerificationRun { manifest })
}

pub fn formal_verification_manifest(
    project_root: impl Into<PathBuf>,
    package_path: &str,
    options: &FormalVerificationOptions,
) -> Result<FormalVerificationManifest, FormalVerificationAdapterError> {
    prepare_manifest(project_root.into(), package_path, options)
}

fn prepare_manifest(
    project_root: PathBuf,
    package_path: &str,
    options: &FormalVerificationOptions,
) -> Result<FormalVerificationManifest, FormalVerificationAdapterError> {
    let project_root = project_root.canonicalize().map_err(|error| {
        FormalVerificationAdapterError::InvalidProjectRoot {
            path: project_root.clone(),
            reason: error.to_string(),
        }
    })?;

    if !project_root.is_dir() {
        return Err(FormalVerificationAdapterError::InvalidProjectRoot {
            path: project_root,
            reason: "path is not a directory".to_string(),
        });
    }

    let package_root = normalize_package_root(&project_root, package_path)?;
    let file_path = normalize_move_file(&project_root, &package_root, &options.file_path)?;
    let module_name = normalize_module_name(&options.module_name)?;

    Ok(FormalVerificationManifest {
        project_root,
        package_root,
        file_path,
        module_name,
        timeout_seconds: options
            .timeout_seconds
            .unwrap_or(DEFAULT_EXECUTION_TIMEOUT_SECONDS),
    })
}

fn normalize_package_root(
    project_root: &Path,
    package_path: &str,
) -> Result<PathBuf, FormalVerificationAdapterError> {
    let package_path = package_path.trim();
    let candidate = if package_path.is_empty() || package_path == "." {
        project_root.to_path_buf()
    } else {
        project_root.join(package_path)
    };
    let package_root = candidate.canonicalize().map_err(|error| {
        FormalVerificationAdapterError::InvalidPackagePath {
            path: candidate.clone(),
            reason: error.to_string(),
        }
    })?;

    if !package_root.starts_with(project_root) {
        return Err(FormalVerificationAdapterError::InvalidPackagePath {
            path: package_root,
            reason: "package path escapes the project root".to_string(),
        });
    }

    if !package_root.join("Move.toml").is_file() {
        return Err(FormalVerificationAdapterError::InvalidPackagePath {
            path: package_root,
            reason: "package does not contain a Move.toml file".to_string(),
        });
    }

    Ok(package_root)
}

fn normalize_move_file(
    project_root: &Path,
    package_root: &Path,
    file_path: &str,
) -> Result<String, FormalVerificationAdapterError> {
    let normalized = file_path.trim().replace('\\', "/");

    if normalized.is_empty() {
        return Err(FormalVerificationAdapterError::InvalidFilePath {
            path: file_path.to_string(),
            reason: "file path cannot be empty".to_string(),
        });
    }

    if !normalized.ends_with(".move") {
        return Err(FormalVerificationAdapterError::InvalidFilePath {
            path: normalized,
            reason: "file must be a .move source file".to_string(),
        });
    }

    let absolute = if Path::new(&normalized).is_absolute() {
        PathBuf::from(&normalized)
    } else {
        project_root.join(&normalized)
    };
    let absolute = absolute.canonicalize().map_err(|error| {
        FormalVerificationAdapterError::InvalidFilePath {
            path: normalized.clone(),
            reason: error.to_string(),
        }
    })?;

    if !absolute.starts_with(package_root) {
        return Err(FormalVerificationAdapterError::InvalidFilePath {
            path: normalized,
            reason: "file is not inside the selected package".to_string(),
        });
    }

    let relative = absolute.strip_prefix(project_root).map_err(|error| {
        FormalVerificationAdapterError::InvalidFilePath {
            path: absolute.display().to_string(),
            reason: error.to_string(),
        }
    })?;

    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_module_name(module_name: &str) -> Result<String, FormalVerificationAdapterError> {
    let module_name = module_name.trim();

    if module_name.is_empty()
        || !module_name.chars().all(|character| {
            character == '_' || character == ':' || character.is_ascii_alphanumeric()
        })
    {
        return Err(FormalVerificationAdapterError::InvalidModuleName(
            module_name.to_string(),
        ));
    }

    Ok(module_name.to_string())
}

fn general_config(options: &FormalVerificationOptions) -> GeneralConfig {
    GeneralConfig {
        timeout: options.timeout_seconds,
        force_timeout: false,
        keep_temp: options.keep_temp,
        generate_only: false,
        verbose: options.verbose,
        no_counterexample_trace: false,
        explain: false,
        use_array_theory: false,
        split_paths: None,
        no_bv_int_encoding: false,
        boogie_file_mode: BoogieFileMode::Function,
        dump_bytecode: false,
        enable_conditional_merge_insertion: false,
        skip_spec_no_abort: false,
        skip_fun_no_abort: false,
        stats: false,
        ci: false,
        trace: options.trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn manifest_normalizes_package_and_file_paths() {
        let project = TempDir::new().expect("tempdir");
        fs::create_dir_all(project.path().join("sources")).expect("sources");
        fs::write(
            project.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::write(
            project.path().join("sources/demo.move"),
            "module demo::demo {}",
        )
        .expect("source");

        let manifest = formal_verification_manifest(
            project.path(),
            ".",
            &FormalVerificationOptions {
                file_path: "sources/demo.move".to_string(),
                module_name: "demo".to_string(),
                ..FormalVerificationOptions::default()
            },
        )
        .expect("manifest");

        assert_eq!(manifest.file_path, "sources/demo.move");
        assert_eq!(manifest.module_name, "demo");
        assert_eq!(manifest.timeout_seconds, DEFAULT_EXECUTION_TIMEOUT_SECONDS);
    }

    #[test]
    fn manifest_rejects_non_move_files() {
        let project = TempDir::new().expect("tempdir");
        fs::write(
            project.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::write(project.path().join("README.md"), "not move").expect("file");

        let error = formal_verification_manifest(
            project.path(),
            ".",
            &FormalVerificationOptions {
                file_path: "README.md".to_string(),
                module_name: "demo".to_string(),
                ..FormalVerificationOptions::default()
            },
        )
        .expect_err("non-move file should fail");

        assert!(error.to_string().contains(".move"));
    }

    #[test]
    fn manifest_rejects_package_escape() {
        let project = TempDir::new().expect("tempdir");
        let outside = TempDir::new().expect("outside");
        fs::write(
            project.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        fs::write(outside.path().join("x.move"), "module demo::x {}").expect("file");

        let error = formal_verification_manifest(
            project.path(),
            ".",
            &FormalVerificationOptions {
                file_path: outside.path().join("x.move").display().to_string(),
                module_name: "demo".to_string(),
                ..FormalVerificationOptions::default()
            },
        )
        .expect_err("escaped file should fail");

        assert!(error.to_string().contains("inside the selected package"));
    }
}
