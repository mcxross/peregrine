use movy::sui::fuzz::{fuzz_local_package, LocalPackageFuzzArgs, LocalPackageFuzzResult};
use serde::Serialize;
use std::{
    any::Any,
    fs,
    path::{Component, Path, PathBuf},
    thread,
};
use walkdir::{DirEntry, WalkDir};

const MOVY_FUZZ_THREAD_STACK_SIZE: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovyFuzzOptions {
    pub time_limit_seconds: u64,
    pub seed: u64,
}

impl Default for MovyFuzzOptions {
    fn default() -> Self {
        Self {
            time_limit_seconds: 30,
            seed: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovyFuzzRun {
    pub manifest: MovyFuzzManifest,
    pub stdout: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovyFuzzManifest {
    pub project_root: String,
    pub package_path: String,
    pub temp_package_root: String,
    pub deployed_package_id: String,
    pub network: String,
    pub graphql_url: String,
    pub package_names: Vec<String>,
    pub public_function_count: usize,
    pub target_functions: Vec<String>,
    pub seed: u64,
    pub time_limit_seconds: u64,
    pub queue_entries: usize,
    pub crash_entries: usize,
}

impl MovyFuzzManifest {
    pub fn summary(&self) -> String {
        let mut lines = vec![
            "Peregrine Movy fuzz adapter".to_string(),
            "Mode: Movy executor fuzzing".to_string(),
            format!("Package: {}", self.package_path),
            format!("Deployed package: {}", self.deployed_package_id),
            format!("Network: {}", self.network),
            format!("GraphQL: {}", self.graphql_url),
            format!("Seed: {}", self.seed),
            format!("Time limit: {}s", self.time_limit_seconds),
            format!("Public targets: {}", self.public_function_count),
            format!("Queue entries: {}", self.queue_entries),
            format!("Crash entries: {}", self.crash_entries),
        ];

        if !self.package_names.is_empty() {
            lines.push(format!("Package names: {}", self.package_names.join(", ")));
        }

        if !self.target_functions.is_empty() {
            lines.push("Targets:".to_string());
            lines.extend(
                self.target_functions
                    .iter()
                    .map(|target| format!("  - {target}")),
            );
        }

        lines.join("\n")
    }
}

#[derive(Debug)]
pub enum MovyFuzzAdapterError {
    Io(String),
    InvalidPackage(String),
    Movy(String),
    Runtime(String),
    Panic(String),
}

impl std::fmt::Display for MovyFuzzAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message)
            | Self::InvalidPackage(message)
            | Self::Movy(message)
            | Self::Runtime(message)
            | Self::Panic(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for MovyFuzzAdapterError {}

type AdapterResult<T> = Result<T, MovyFuzzAdapterError>;

pub fn run_movy_fuzz_blocking(
    project_root: impl AsRef<Path>,
    package_path: &str,
    options: MovyFuzzOptions,
) -> AdapterResult<MovyFuzzRun> {
    let project_root = project_root.as_ref().to_path_buf();
    let package_path = package_path.to_owned();

    let worker = thread::Builder::new()
        .name("peregrine-movy-fuzz".to_string())
        .stack_size(MOVY_FUZZ_THREAD_STACK_SIZE)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("peregrine-movy-fuzz-runtime")
                .thread_stack_size(MOVY_FUZZ_THREAD_STACK_SIZE)
                .build()
                .map_err(|error| {
                    MovyFuzzAdapterError::Runtime(format!(
                        "Could not create Movy fuzz runtime: {error}"
                    ))
                })?;

            runtime.block_on(run_movy_fuzz(project_root, &package_path, options))
        })
        .map_err(|error| {
            MovyFuzzAdapterError::Runtime(format!(
                "Could not start Movy fuzz worker thread: {error}"
            ))
        })?;

    worker.join().map_err(panic_payload_to_error)?
}

fn panic_payload_to_error(payload: Box<dyn Any + Send + 'static>) -> MovyFuzzAdapterError {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return MovyFuzzAdapterError::Panic(format!("Movy fuzz worker panicked: {message}"));
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return MovyFuzzAdapterError::Panic(format!("Movy fuzz worker panicked: {message}"));
    }

    MovyFuzzAdapterError::Panic("Movy fuzz worker panicked".to_string())
}

pub async fn run_movy_fuzz(
    project_root: impl AsRef<Path>,
    package_path: &str,
    options: MovyFuzzOptions,
) -> AdapterResult<MovyFuzzRun> {
    let project_root = canonicalize_project_root(project_root.as_ref())?;
    let package_relative_path = normalize_package_path(package_path)?;
    let package_root = canonicalize_package_root(&project_root, &package_relative_path)?;

    let temp_dir = tempfile::tempdir().map_err(|error| {
        MovyFuzzAdapterError::Io(format!("Could not create fuzz tempdir: {error}"))
    })?;
    copy_project_workspace(&project_root, temp_dir.path())?;

    let temp_package_root = temp_dir.path().join(&package_relative_path);
    let output_root = temp_dir.path().join("movy-fuzz-output");

    let mut args = LocalPackageFuzzArgs::new(temp_package_root.clone());
    args.seed = Some(options.seed);
    args.time_limit = Some(options.time_limit_seconds);
    args.output = Some(output_root.clone());

    let result = fuzz_local_package(args)
        .await
        .map_err(|error| MovyFuzzAdapterError::Movy(error.to_string()))?;

    let manifest = build_manifest(
        &project_root,
        &package_relative_path,
        &temp_package_root,
        &output_root,
        &result,
        options,
    );
    let stdout = manifest.summary();

    drop(package_root);
    drop(temp_dir);

    Ok(MovyFuzzRun { manifest, stdout })
}

fn build_manifest(
    project_root: &Path,
    package_relative_path: &Path,
    temp_package_root: &Path,
    output_root: &Path,
    result: &LocalPackageFuzzResult,
    options: MovyFuzzOptions,
) -> MovyFuzzManifest {
    MovyFuzzManifest {
        project_root: project_root.to_string_lossy().into_owned(),
        package_path: display_package_path(package_relative_path),
        temp_package_root: temp_package_root.to_string_lossy().into_owned(),
        deployed_package_id: result.package_id.to_string(),
        network: result.network.clone(),
        graphql_url: result.graphql_url.clone(),
        package_names: result.package_names.clone(),
        public_function_count: result.target_functions.len(),
        target_functions: result.target_functions.clone(),
        seed: result.seed,
        time_limit_seconds: result.time_limit.unwrap_or(options.time_limit_seconds),
        queue_entries: count_directory_entries(&output_root.join("queue")),
        crash_entries: count_directory_entries(&output_root.join("crashes")),
    }
}

fn canonicalize_project_root(project_root: &Path) -> AdapterResult<PathBuf> {
    let root = project_root.canonicalize().map_err(|error| {
        MovyFuzzAdapterError::InvalidPackage(format!(
            "Could not read project directory {}: {error}",
            project_root.display()
        ))
    })?;

    if !root.is_dir() {
        return Err(MovyFuzzAdapterError::InvalidPackage(format!(
            "Project root {} is not a directory.",
            root.display()
        )));
    }

    Ok(root)
}

fn canonicalize_package_root(project_root: &Path, package_path: &Path) -> AdapterResult<PathBuf> {
    let package_root = project_root
        .join(package_path)
        .canonicalize()
        .map_err(|error| {
            MovyFuzzAdapterError::InvalidPackage(format!(
                "Could not read Move package {}: {error}",
                project_root.join(package_path).display()
            ))
        })?;

    if !package_root.starts_with(project_root) {
        return Err(MovyFuzzAdapterError::InvalidPackage(
            "Move package must be inside the opened project.".to_string(),
        ));
    }

    if !package_root.is_dir() {
        return Err(MovyFuzzAdapterError::InvalidPackage(
            "Selected package path is not a directory.".to_string(),
        ));
    }

    if !package_root.join("Move.toml").is_file() {
        return Err(MovyFuzzAdapterError::InvalidPackage(
            "Selected package does not contain a Move.toml file.".to_string(),
        ));
    }

    Ok(package_root)
}

fn normalize_package_path(package_path: &str) -> AdapterResult<PathBuf> {
    let raw = package_path.trim();
    if Path::new(raw).is_absolute() {
        return Err(MovyFuzzAdapterError::InvalidPackage(
            "Use a package path relative to the opened project.".to_string(),
        ));
    }

    let trimmed = raw.trim_matches('/');

    if trimmed.is_empty() || trimmed == "." {
        return Ok(PathBuf::new());
    }

    let path = Path::new(trimmed);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(MovyFuzzAdapterError::InvalidPackage(
            "Use a package path relative to the opened project.".to_string(),
        ));
    }

    Ok(path.to_path_buf())
}

fn display_package_path(package_path: &Path) -> String {
    if package_path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        package_path.to_string_lossy().into_owned()
    }
}

fn copy_project_workspace(project_root: &Path, target_root: &Path) -> AdapterResult<()> {
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|entry| !should_skip_walk_entry(entry))
    {
        let entry = entry.map_err(|error| {
            MovyFuzzAdapterError::Io(format!(
                "Could not walk project directory {}: {error}",
                project_root.display()
            ))
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(project_root).map_err(|error| {
            MovyFuzzAdapterError::Io(format!(
                "Could not make {} relative to {}: {error}",
                path.display(),
                project_root.display()
            ))
        })?;

        if relative.as_os_str().is_empty() {
            continue;
        }

        let destination = target_root.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).map_err(|error| {
                MovyFuzzAdapterError::Io(format!(
                    "Could not create directory {}: {error}",
                    destination.display()
                ))
            })?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    MovyFuzzAdapterError::Io(format!(
                        "Could not create directory {}: {error}",
                        parent.display()
                    ))
                })?;
            }
            fs::copy(path, &destination).map_err(|error| {
                MovyFuzzAdapterError::Io(format!(
                    "Could not copy {} to {}: {error}",
                    path.display(),
                    destination.display()
                ))
            })?;
        }
    }

    Ok(())
}

fn should_skip_walk_entry(entry: &DirEntry) -> bool {
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };

    matches!(
        name,
        ".git"
            | ".move"
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

fn count_directory_entries(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absolute_package_paths() {
        assert!(normalize_package_path("/tmp/demo").is_err());
        assert!(normalize_package_path("../demo").is_err());
        assert_eq!(normalize_package_path(".").unwrap(), PathBuf::new());
    }
}
