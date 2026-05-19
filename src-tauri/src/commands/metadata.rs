use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

const PROJECT_METADATA_DIRECTORY: &str = ".peregrine";
const PROJECT_METADATA_FILE: &str = "metadata.json";

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectMetadata {
    #[serde(default = "default_project_metadata_version")]
    version: u32,
    #[serde(default)]
    agents: Option<serde_json::Value>,
    #[serde(default)]
    builds: HashMap<String, ProjectBuildMetadata>,
    #[serde(default)]
    package_configs: HashMap<String, ProjectPackageConfig>,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            version: default_project_metadata_version(),
            agents: None,
            builds: HashMap::new(),
            package_configs: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectBuildMetadata {
    last_successful_build_at: Option<u64>,
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectPackageConfig {
    #[serde(default)]
    commands: ProjectCommandConfig,
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectCommandConfig {
    move_coverage_script_path: Option<String>,
    move_test_script_path: Option<String>,
}

#[tauri::command]
pub(crate) async fn load_project_metadata(root_path: String) -> Result<ProjectMetadata, String> {
    tauri::async_runtime::spawn_blocking(move || read_project_metadata(&root_path))
        .await
        .map_err(|error| format!("Could not join project metadata load task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_project_metadata(
    root_path: String,
    metadata: ProjectMetadata,
) -> Result<ProjectMetadata, String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_project_metadata(&root_path, &metadata)?;
        Ok(metadata)
    })
    .await
    .map_err(|error| format!("Could not join project metadata save task: {error}"))?
}

fn default_project_metadata_version() -> u32 {
    1
}

fn read_project_metadata(root_path: &str) -> Result<ProjectMetadata, String> {
    let path = project_metadata_path(root_path)?;

    if !path.exists() {
        return Ok(ProjectMetadata::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Could not read project metadata {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "Could not parse project metadata {}: {error}",
            path.display()
        )
    })
}

fn write_project_metadata(root_path: &str, metadata: &ProjectMetadata) -> Result<(), String> {
    let path = project_metadata_path(root_path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create project metadata directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let contents = serde_json::to_string_pretty(metadata)
        .map_err(|error| format!("Could not serialize project metadata: {error}"))?;

    fs::write(&path, format!("{contents}\n")).map_err(|error| {
        format!(
            "Could not write project metadata {}: {error}",
            path.display()
        )
    })
}

fn project_metadata_path(root_path: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read package directory {root_path}: {error}"))?;

    Ok(root
        .join(PROJECT_METADATA_DIRECTORY)
        .join(PROJECT_METADATA_FILE))
}
