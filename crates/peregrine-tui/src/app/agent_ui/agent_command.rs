use peregrine_app_server_protocol::AgentRoleSaveScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentCommand {
    RolesList,
    RoleShow {
        name: String,
    },
    RoleEdit {
        name: String,
        scope: AgentRoleSaveScope,
    },
    RoleNew {
        name: String,
        scope: AgentRoleSaveScope,
    },
}

pub(crate) const AGENT_USAGE: &str = "Usage: /agent [roles [role-name] | roles edit <role-name> [--global|--local] | roles new <role-name> [--global|--local]]";

pub(crate) fn parse_agent_command(args: &str) -> Result<AgentCommand, String> {
    let mut parts = args.split_whitespace();
    match parts.next() {
        Some("roles") | Some("role") => parse_roles_command(parts.collect()),
        Some(other) => Err(format!(
            "Unknown /agent subcommand `{other}`. Use `/agent roles` to inspect configured roles."
        )),
        None => Err(AGENT_USAGE.to_string()),
    }
}

fn parse_roles_command(parts: Vec<&str>) -> Result<AgentCommand, String> {
    match parts.as_slice() {
        [] => Ok(AgentCommand::RolesList),
        ["new", rest @ ..] => {
            let (name, scope) = parse_role_name_and_scope("new", rest)?;
            Ok(AgentCommand::RoleNew { name, scope })
        }
        ["edit", rest @ ..] => {
            let (name, scope) = parse_role_name_and_scope("edit", rest)?;
            Ok(AgentCommand::RoleEdit { name, scope })
        }
        [name] => Ok(AgentCommand::RoleShow {
            name: (*name).to_string(),
        }),
        _ => Err(AGENT_USAGE.to_string()),
    }
}

fn parse_role_name_and_scope(
    subcommand: &str,
    parts: &[&str],
) -> Result<(String, AgentRoleSaveScope), String> {
    let mut name = None;
    let mut scope = None;
    for part in parts {
        match *part {
            "--global" => set_scope(&mut scope, AgentRoleSaveScope::Global)?,
            "--local" => set_scope(&mut scope, AgentRoleSaveScope::Local)?,
            flag if flag.starts_with("--") => {
                return Err(format!(
                    "Unknown /agent roles {subcommand} option `{flag}`. Use --global or --local."
                ));
            }
            value if name.is_none() => name = Some(value.to_string()),
            _ => {
                return Err(format!(
                    "Usage: /agent roles {subcommand} <role-name> [--global|--local]"
                ));
            }
        }
    }

    let name = name.ok_or_else(|| {
        format!("Usage: /agent roles {subcommand} <role-name> [--global|--local]")
    })?;
    Ok((name, scope.unwrap_or(AgentRoleSaveScope::Global)))
}

fn set_scope(
    scope: &mut Option<AgentRoleSaveScope>,
    next: AgentRoleSaveScope,
) -> Result<(), String> {
    if let Some(previous) = scope
        && *previous != next
    {
        return Err("Use only one of --global or --local.".to_string());
    }
    *scope = Some(next);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_roles_list() {
        assert_eq!(parse_agent_command("roles"), Ok(AgentCommand::RolesList));
    }

    #[test]
    fn parses_agent_role_show() {
        assert_eq!(
            parse_agent_command("roles custom-role"),
            Ok(AgentCommand::RoleShow {
                name: "custom-role".to_string()
            })
        );
    }

    #[test]
    fn parses_agent_role_edit() {
        assert_eq!(
            parse_agent_command("roles edit custom-role"),
            Ok(AgentCommand::RoleEdit {
                name: "custom-role".to_string(),
                scope: AgentRoleSaveScope::Global
            })
        );
    }

    #[test]
    fn parses_agent_role_edit_local_scope() {
        assert_eq!(
            parse_agent_command("roles edit custom-role --local"),
            Ok(AgentCommand::RoleEdit {
                name: "custom-role".to_string(),
                scope: AgentRoleSaveScope::Local
            })
        );
    }

    #[test]
    fn parses_agent_role_new_global_scope() {
        assert_eq!(
            parse_agent_command("roles new custom-role --global"),
            Ok(AgentCommand::RoleNew {
                name: "custom-role".to_string(),
                scope: AgentRoleSaveScope::Global
            })
        );
    }
}
