use crate::config_manager::ConfigManager;
use crate::config_manager_service::ConfigManagerError;
use codex_utils_absolute_path::AbsolutePathBuf;
use peregrine_app_server_protocol::AgentRoleReadParams;
use peregrine_app_server_protocol::AgentRoleReadResponse;
use peregrine_app_server_protocol::AgentRoleSaveScope;
use peregrine_app_server_protocol::AgentRoleSource;
use peregrine_app_server_protocol::AgentRoleSummary;
use peregrine_app_server_protocol::AgentRoleWriteParams;
use peregrine_app_server_protocol::AgentRoleWriteResponse;
use peregrine_app_server_protocol::ConfigBatchWriteParams;
use peregrine_app_server_protocol::ConfigEdit as ApiConfigEdit;
use peregrine_app_server_protocol::ConfigLayerSource;
use peregrine_app_server_protocol::ConfigWriteErrorCode;
use peregrine_app_server_protocol::ConfigWriteResponse;
use peregrine_app_server_protocol::MergeStrategy;
use peregrine_app_server_protocol::WriteStatus;
use peregrine_config::CONFIG_TOML_FILE;
use peregrine_config::ConfigLayerEntry;
use peregrine_core::agent_role_catalog::AgentRoleCatalogEntry;
use peregrine_core::agent_role_catalog::AgentRoleCatalogSource;
use peregrine_core::agent_role_catalog::built_in_role_config_file_contents;
use peregrine_core::agent_role_catalog::list_agent_roles;
use peregrine_core::agent_role_catalog::parse_agent_role_file_contents;
use peregrine_core::path_utils::write_atomically;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use tokio::task;
use toml::Value as TomlValue;
use toml_edit::Array;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::value;

const ROLE_FILE_DIR: &str = "agents";

#[derive(Debug, Clone)]
struct RoleSaveTarget {
    scope: AgentRoleSaveScope,
    config_file: PathBuf,
    role_file: PathBuf,
    version: Option<String>,
}

impl ConfigManager {
    pub(crate) async fn read_agent_role_edit(
        &self,
        params: AgentRoleReadParams,
    ) -> Result<AgentRoleReadResponse, ConfigManagerError> {
        let role_name = validate_role_name(&params.name)?;
        let cwd = params.cwd.map(PathBuf::from);
        let scope = params.scope.unwrap_or(AgentRoleSaveScope::Global);
        let target = self
            .resolve_role_save_target(scope.clone(), cwd.as_deref(), role_name)
            .await?;
        let global_config_file = self
            .user_config_path()
            .map_err(|err| ConfigManagerError::io("failed to resolve user config path", err))?;
        let directory_config_file = cwd
            .as_deref()
            .map(directory_config_file_for_cwd)
            .transpose()?;

        if params.create {
            let config = self
                .load_latest_config(cwd.clone())
                .await
                .map_err(|err| ConfigManagerError::io("failed to load configuration", err))?;
            if list_agent_roles(&config)
                .iter()
                .any(|entry| entry.name == role_name)
            {
                return Err(ConfigManagerError::write(
                    ConfigWriteErrorCode::ConfigValidationError,
                    format!("agent role `{role_name}` already exists"),
                ));
            }

            return Ok(AgentRoleReadResponse {
                name: role_name.to_string(),
                source: None,
                scope,
                config_file: target.role_file.to_string_lossy().into_owned(),
                global_config_file: global_config_file.as_path().display().to_string(),
                directory_config_file: directory_config_file
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                editable_content: new_role_template(role_name),
                save_config_file: target.config_file.to_string_lossy().into_owned(),
                save_config_version: target.version,
                create: true,
                overrides_built_in: false,
            });
        }

        let config = self
            .load_latest_config(cwd.clone())
            .await
            .map_err(|err| ConfigManagerError::io("failed to load configuration", err))?;
        let entry = list_agent_roles(&config)
            .into_iter()
            .find(|entry| entry.name == role_name)
            .ok_or_else(|| {
                ConfigManagerError::write(
                    ConfigWriteErrorCode::ConfigValidationError,
                    format!("unknown agent role `{role_name}`"),
                )
            })?;

        Ok(AgentRoleReadResponse {
            name: role_name.to_string(),
            source: Some(agent_role_source(&entry.source)),
            scope,
            config_file: target.role_file.to_string_lossy().into_owned(),
            global_config_file: global_config_file.as_path().display().to_string(),
            directory_config_file: directory_config_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            editable_content: editable_content_for_role(&entry).await,
            save_config_file: target.config_file.to_string_lossy().into_owned(),
            save_config_version: target.version,
            create: false,
            overrides_built_in: entry.source == AgentRoleCatalogSource::BuiltIn
                || entry.overrides_built_in,
        })
    }

    pub(crate) async fn write_agent_role_edit(
        &self,
        params: AgentRoleWriteParams,
    ) -> Result<AgentRoleWriteResponse, ConfigManagerError> {
        let role_name = validate_role_name(&params.name)?;
        let cwd = params.cwd.clone().map(PathBuf::from);
        let config = self
            .load_latest_config(cwd.clone())
            .await
            .map_err(|err| ConfigManagerError::io("failed to load configuration", err))?;
        let role_exists = list_agent_roles(&config)
            .iter()
            .any(|entry| entry.name == role_name);
        if params.create && role_exists {
            return Err(ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("agent role `{role_name}` already exists"),
            ));
        }
        if !params.create && !role_exists {
            return Err(ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("unknown agent role `{role_name}`"),
            ));
        }

        let target = self
            .resolve_role_save_target(params.scope.clone(), cwd.as_deref(), role_name)
            .await?;
        validate_role_file_contents(role_name, &target.role_file, &params.editable_content)?;
        write_role_file(&target.role_file, params.editable_content).await?;
        let config_write = self
            .write_role_declaration(&target, role_name, params.expected_version)
            .await?;

        let next_config = self
            .load_latest_config(cwd)
            .await
            .map_err(|err| ConfigManagerError::io("failed to load updated configuration", err))?;
        let role = list_agent_roles(&next_config)
            .into_iter()
            .find(|entry| entry.name == role_name)
            .ok_or_else(|| {
                ConfigManagerError::write(
                    ConfigWriteErrorCode::ConfigValidationError,
                    format!("agent role `{role_name}` was not available after write"),
                )
            })?;

        Ok(AgentRoleWriteResponse {
            role: agent_role_summary_from_catalog_entry(role),
            config_file: target.role_file.to_string_lossy().into_owned(),
            config_write,
        })
    }

    async fn resolve_role_save_target(
        &self,
        scope: AgentRoleSaveScope,
        cwd: Option<&Path>,
        role_name: &str,
    ) -> Result<RoleSaveTarget, ConfigManagerError> {
        match scope {
            AgentRoleSaveScope::Global => {
                let config_file = self.user_config_path().map_err(|err| {
                    ConfigManagerError::io("failed to resolve user config path", err)
                })?;
                let version = self.global_config_version().await?;
                Ok(RoleSaveTarget {
                    scope,
                    config_file: config_file.as_path().to_path_buf(),
                    role_file: role_file_path_for_config_file(config_file.as_path(), role_name)?,
                    version,
                })
            }
            AgentRoleSaveScope::Local => {
                let cwd = cwd.ok_or_else(|| {
                    ConfigManagerError::write(
                        ConfigWriteErrorCode::ConfigValidationError,
                        "local agent roles require a cwd",
                    )
                })?;
                let config_file = directory_config_file_for_cwd(cwd)?;
                let version = self.directory_config_version(cwd, &config_file).await?;
                Ok(RoleSaveTarget {
                    scope,
                    config_file: config_file.clone(),
                    role_file: role_file_path_for_config_file(&config_file, role_name)?,
                    version,
                })
            }
        }
    }

    async fn write_role_declaration(
        &self,
        target: &RoleSaveTarget,
        role_name: &str,
        expected_version: Option<String>,
    ) -> Result<ConfigWriteResponse, ConfigManagerError> {
        match target.scope {
            AgentRoleSaveScope::Global => {
                self.batch_write(ConfigBatchWriteParams {
                    edits: vec![role_declaration_edit(role_name, &target.role_file)],
                    file_path: None,
                    expected_version,
                    reload_user_config: false,
                })
                .await
            }
            AgentRoleSaveScope::Local => {
                if let Some(expected_version) = expected_version.as_deref()
                    && Some(expected_version) != target.version.as_deref()
                {
                    return Err(ConfigManagerError::write(
                        ConfigWriteErrorCode::ConfigVersionConflict,
                        "Local configuration was modified since last read. Fetch latest version and retry.",
                    ));
                }
                write_directory_role_declaration(&target.config_file, role_name).await?;
                Ok(ConfigWriteResponse {
                    status: WriteStatus::Ok,
                    version: version_for_config_file(
                        &target.config_file,
                        ConfigLayerSource::Project {
                            dot_peregrine_folder: absolute_path(
                                target.config_file.parent().ok_or_else(|| {
                                    ConfigManagerError::write(
                                        ConfigWriteErrorCode::ConfigValidationError,
                                        "directory config path has no parent directory",
                                    )
                                })?,
                            )?,
                        },
                    )
                    .await?,
                    file_path: absolute_path(&target.config_file)?,
                    overridden_metadata: None,
                })
            }
        }
    }

    async fn global_config_version(&self) -> Result<Option<String>, ConfigManagerError> {
        let layers = self
            .load_config_layers(/*cwd*/ None)
            .await
            .map_err(|err| ConfigManagerError::io("failed to load configuration", err))?;
        Ok(layers
            .get_active_user_layer()
            .map(|layer| layer.version.clone()))
    }

    async fn directory_config_version(
        &self,
        cwd: &Path,
        config_file: &Path,
    ) -> Result<Option<String>, ConfigManagerError> {
        let layers = self
            .load_config_layers(Some(absolute_path(cwd)?))
            .await
            .map_err(|err| ConfigManagerError::io("failed to load configuration", err))?;
        let Some(dot_peregrine_folder) = config_file.parent() else {
            return Ok(None);
        };
        Ok(layers
            .layers_high_to_low()
            .into_iter()
            .find_map(|layer| match &layer.name {
                ConfigLayerSource::Project {
                    dot_peregrine_folder: folder,
                } if folder.as_path() == dot_peregrine_folder => Some(layer.version.clone()),
                _ => None,
            }))
    }
}

fn agent_role_summary_from_catalog_entry(entry: AgentRoleCatalogEntry) -> AgentRoleSummary {
    AgentRoleSummary {
        name: entry.name,
        description: entry.description,
        source: agent_role_source(&entry.source),
        config_file: entry
            .config_file
            .map(|path| path.to_string_lossy().into_owned()),
        nickname_candidates: entry.nickname_candidates,
        overrides_built_in: entry.overrides_built_in,
    }
}

fn agent_role_source(source: &AgentRoleCatalogSource) -> AgentRoleSource {
    match source {
        AgentRoleCatalogSource::BuiltIn => AgentRoleSource::BuiltIn,
        AgentRoleCatalogSource::Configured => AgentRoleSource::Configured,
    }
}

fn validate_role_name(name: &str) -> Result<&str, ConfigManagerError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ConfigManagerError::write(
            ConfigWriteErrorCode::ConfigValidationError,
            "agent role name cannot be empty",
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(ConfigManagerError::write(
            ConfigWriteErrorCode::ConfigValidationError,
            "agent role names edited from the TUI may only contain ASCII letters, digits, hyphens, and underscores",
        ));
    }
    Ok(name)
}

fn role_file_path_for_config_file(
    config_file: &Path,
    role_name: &str,
) -> Result<PathBuf, ConfigManagerError> {
    let Some(config_dir) = config_file.parent() else {
        return Err(ConfigManagerError::write(
            ConfigWriteErrorCode::ConfigValidationError,
            "config path has no parent directory",
        ));
    };
    Ok(config_dir
        .join(ROLE_FILE_DIR)
        .join(format!("{role_name}.toml")))
}

fn directory_config_file_for_cwd(cwd: &Path) -> Result<PathBuf, ConfigManagerError> {
    Ok(absolute_path(cwd)?
        .join(".peregrine")
        .join(CONFIG_TOML_FILE)
        .as_path()
        .to_path_buf())
}

fn role_declaration_edit(role_name: &str, role_file: &Path) -> ApiConfigEdit {
    ApiConfigEdit {
        key_path: format!("agents.{role_name}"),
        value: json!({
            "config_file": role_file.to_string_lossy()
        }),
        merge_strategy: MergeStrategy::Replace,
    }
}

async fn write_directory_role_declaration(
    config_file: &Path,
    role_name: &str,
) -> Result<(), ConfigManagerError> {
    let config_file = config_file.to_path_buf();
    let role_name = role_name.to_string();
    task::spawn_blocking(move || {
        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut doc = match std::fs::read_to_string(&config_file) {
            Ok(contents) => contents
                .parse::<DocumentMut>()
                .map_err(std::io::Error::other)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
            Err(err) => return Err(err),
        };
        doc["agents"][&role_name]["config_file"] =
            value(format!("./{ROLE_FILE_DIR}/{role_name}.toml"));
        write_atomically(&config_file, &doc.to_string())
    })
    .await
    .map_err(|err| {
        ConfigManagerError::anyhow("directory config persistence task panicked", err.into())
    })?
    .map_err(|err| ConfigManagerError::io("failed to write directory config", err))
}

async fn version_for_config_file(
    config_file: &Path,
    source: ConfigLayerSource,
) -> Result<String, ConfigManagerError> {
    let contents = tokio::fs::read_to_string(config_file)
        .await
        .map_err(|err| ConfigManagerError::io("failed to read updated config", err))?;
    let config: TomlValue = toml::from_str(&contents)
        .map_err(|err| ConfigManagerError::toml("failed to parse updated config", err))?;
    Ok(ConfigLayerEntry::new(source, config).version)
}

fn absolute_path(path: &Path) -> Result<AbsolutePathBuf, ConfigManagerError> {
    AbsolutePathBuf::from_absolute_path(path).map_err(|err| {
        ConfigManagerError::io("failed to resolve config path to an absolute path", err)
    })
}

async fn editable_content_for_role(entry: &AgentRoleCatalogEntry) -> String {
    let base_content = match entry.source {
        AgentRoleCatalogSource::BuiltIn => entry
            .config_file
            .as_deref()
            .and_then(built_in_role_config_file_contents)
            .map(ToOwned::to_owned),
        AgentRoleCatalogSource::Configured => match entry.config_file.as_deref() {
            Some(path) => tokio::fs::read_to_string(path).await.ok(),
            None => None,
        },
    };
    let base_content = base_content.unwrap_or_default();
    role_content_with_metadata(
        &entry.name,
        entry.description.as_deref(),
        entry.nickname_candidates.as_deref(),
        &base_content,
    )
}

fn role_content_with_metadata(
    role_name: &str,
    description: Option<&str>,
    nickname_candidates: Option<&[String]>,
    base_content: &str,
) -> String {
    let mut doc = base_content
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());
    doc["name"] = value(role_name);
    if let Some(description) = description {
        doc["description"] = value(description);
    }
    if let Some(nickname_candidates) = nickname_candidates {
        let mut array = Array::new();
        for nickname in nickname_candidates {
            array.push(nickname.as_str());
        }
        doc["nickname_candidates"] = TomlItem::Value(array.into());
    }
    let mut content = doc.to_string();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content
}

fn new_role_template(role_name: &str) -> String {
    format!(
        r#"name = "{role_name}"
description = "Describe when Peregrine should use this agent role."

# Optional role-specific runtime overrides:
# model_provider = "openai"
# model = "gpt-5"
# model_reasoning_effort = "high"
# service_tier = "default"

developer_instructions = """
Describe how this agent should behave, what it owns, and what evidence or
handoff format it must return.
"""
"#
    )
}

fn validate_role_file_contents(
    expected_name: &str,
    role_file: &Path,
    contents: &str,
) -> Result<(), ConfigManagerError> {
    let config_base_dir = role_file.parent().unwrap_or_else(|| Path::new("."));
    let parsed =
        parse_agent_role_file_contents(contents, role_file, config_base_dir, Some(expected_name))
            .map_err(|err| {
            ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid agent role file: {err}"),
            )
        })?;
    if parsed.role_name != expected_name {
        return Err(ConfigManagerError::write(
            ConfigWriteErrorCode::ConfigValidationError,
            format!(
                "agent role file name `{}` does not match requested role `{expected_name}`",
                parsed.role_name
            ),
        ));
    }
    if parsed.description.is_none() {
        return Err(ConfigManagerError::write(
            ConfigWriteErrorCode::ConfigValidationError,
            "agent role file must define a non-empty `description`",
        ));
    }
    Ok(())
}

async fn write_role_file(path: &Path, contents: String) -> Result<(), ConfigManagerError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| ConfigManagerError::io("failed to create agent role directory", err))?;
    }
    let path = path.to_path_buf();
    task::spawn_blocking(move || write_atomically(&path, &contents))
        .await
        .map_err(|err| {
            ConfigManagerError::anyhow("agent role persistence task panicked", err.into())
        })?
        .map_err(|err| ConfigManagerError::io("failed to write agent role file", err))
}
