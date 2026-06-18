use crate::agent::role;
use crate::config::AgentRoleConfig;
use crate::config::Config;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRoleCatalogSource {
    BuiltIn,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRoleCatalogEntry {
    pub name: String,
    pub description: Option<String>,
    pub source: AgentRoleCatalogSource,
    pub config_file: Option<PathBuf>,
    pub nickname_candidates: Option<Vec<String>>,
    pub overrides_built_in: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAgentRoleFile {
    pub role_name: String,
    pub description: Option<String>,
    pub nickname_candidates: Option<Vec<String>>,
}

pub fn list_agent_roles(config: &Config) -> Vec<AgentRoleCatalogEntry> {
    let built_ins = role::built_in_role_configs();
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();

    for (name, role) in &config.agent_roles {
        seen.insert(name.as_str());
        entries.push(entry_from_config(
            name,
            role,
            AgentRoleCatalogSource::Configured,
            built_ins.contains_key(name),
        ));
    }

    for (name, role) in built_ins {
        if seen.insert(name.as_str()) {
            entries.push(entry_from_config(
                name,
                role,
                AgentRoleCatalogSource::BuiltIn,
                /*overrides_built_in*/ false,
            ));
        }
    }

    entries
}

pub fn parse_agent_role_file_contents(
    contents: &str,
    role_file_label: &Path,
    config_base_dir: &Path,
    role_name_hint: Option<&str>,
) -> std::io::Result<ParsedAgentRoleFile> {
    let parsed = crate::config::agent_roles::parse_agent_role_file_contents(
        contents,
        role_file_label,
        config_base_dir,
        role_name_hint,
    )?;
    Ok(ParsedAgentRoleFile {
        role_name: parsed.role_name,
        description: parsed.description,
        nickname_candidates: parsed.nickname_candidates,
    })
}

pub fn built_in_role_config_file_contents(path: &Path) -> Option<&'static str> {
    role::built_in_role_config_file_contents(path)
}

fn entry_from_config(
    name: &str,
    role: &AgentRoleConfig,
    source: AgentRoleCatalogSource,
    overrides_built_in: bool,
) -> AgentRoleCatalogEntry {
    AgentRoleCatalogEntry {
        name: name.to_string(),
        description: role.description.clone(),
        source,
        config_file: role.config_file.clone(),
        nickname_candidates: role.nickname_candidates.clone(),
        overrides_built_in,
    }
}
