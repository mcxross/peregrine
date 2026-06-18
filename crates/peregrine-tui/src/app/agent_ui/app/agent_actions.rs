use super::App;
use crate::agent::agent_command::AgentCommand;
use crate::agent::app_server_session::AppServerSession;
use crate::agent::external_editor;
use crate::agent::tui;
use peregrine_app_server_protocol::AgentRoleListParams;
use peregrine_app_server_protocol::AgentRoleListResponse;
use peregrine_app_server_protocol::AgentRoleReadParams;
use peregrine_app_server_protocol::AgentRoleSaveScope;
use peregrine_app_server_protocol::AgentRoleSource;
use peregrine_app_server_protocol::AgentRoleSummary;
use peregrine_app_server_protocol::AgentRoleWriteParams;
use peregrine_app_server_protocol::WriteStatus;
use ratatui::style::Stylize;
use ratatui::text::Line;

impl App {
    pub(super) async fn handle_agent_command(
        &mut self,
        app_server: &mut AppServerSession,
        tui: &mut tui::Tui,
        command: AgentCommand,
        command_text: String,
    ) {
        self.chat_widget
            .add_plain_history_lines(vec![command_text.magenta().into()]);
        match command {
            AgentCommand::RolesList => self.handle_agent_roles_list(app_server).await,
            AgentCommand::RoleShow { name } => self.handle_agent_role_show(app_server, name).await,
            AgentCommand::RoleEdit { name, scope } => {
                self.handle_agent_role_edit(app_server, tui, name, scope, /*create*/ false)
                    .await;
            }
            AgentCommand::RoleNew { name, scope } => {
                self.handle_agent_role_edit(app_server, tui, name, scope, /*create*/ true)
                    .await;
            }
        }
    }

    async fn handle_agent_roles_list(&mut self, app_server: &mut AppServerSession) {
        match self.agent_role_list(app_server).await {
            Ok(response) => self
                .chat_widget
                .add_plain_history_lines(agent_roles_list_lines(&response)),
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to load agent roles: {err}")),
        }
    }

    async fn handle_agent_role_show(&mut self, app_server: &mut AppServerSession, name: String) {
        match self.agent_role_list(app_server).await {
            Ok(response) => {
                if let Some(role) = response.roles.iter().find(|role| role.name == name) {
                    self.chat_widget
                        .add_plain_history_lines(agent_role_detail_lines(role));
                } else {
                    self.chat_widget.add_error_message(format!(
                        "Unknown agent role `{name}`. Use `/agent roles` to list roles."
                    ));
                }
            }
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to load agent roles: {err}")),
        }
    }

    async fn agent_role_list(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> color_eyre::eyre::Result<AgentRoleListResponse> {
        app_server
            .agent_role_list(AgentRoleListParams {
                cwd: Some(self.config.cwd.as_path().display().to_string()),
            })
            .await
    }

    async fn handle_agent_role_edit(
        &mut self,
        app_server: &mut AppServerSession,
        tui: &mut tui::Tui,
        name: String,
        scope: AgentRoleSaveScope,
        create: bool,
    ) {
        let editor_cmd = match external_editor::resolve_editor_command() {
            Ok(cmd) => cmd,
            Err(external_editor::EditorError::MissingEditor) => {
                self.chat_widget.add_error_message(
                    "Cannot open agent role editor: set $VISUAL or $EDITOR before starting Peregrine."
                        .to_string(),
                );
                return;
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to open agent role editor: {err}"));
                return;
            }
        };

        let cwd = Some(self.config.cwd.as_path().display().to_string());
        let read_response = match app_server
            .agent_role_read(AgentRoleReadParams {
                name: name.clone(),
                cwd: cwd.clone(),
                create,
                scope: Some(scope),
            })
            .await
        {
            Ok(response) => response,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to load agent role `{name}`: {err}"));
                return;
            }
        };

        self.chat_widget.add_info_message(
            format!(
                "Opening {} agent role `{name}` from {} scope.",
                if create { "new" } else { "existing" },
                role_scope_label(scope)
            ),
            Some(format!(
                "Role file: {}\nConfig file: {}",
                read_response.config_file, read_response.save_config_file
            )),
        );

        let editor_result = tui
            .with_restored(tui::RestoreMode::KeepRaw, || async {
                external_editor::run_editor(&read_response.editable_content, &editor_cmd).await
            })
            .await;
        tui.frame_requester().schedule_frame();

        let edited_content = match editor_result {
            Ok(content) => content,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to edit agent role `{name}`: {err}"));
                return;
            }
        };
        if edited_content == read_response.editable_content {
            self.chat_widget
                .add_info_message(format!("No changes saved for agent role `{name}`."), None);
            return;
        }

        match app_server
            .agent_role_write(AgentRoleWriteParams {
                name: name.clone(),
                editable_content: edited_content,
                scope,
                cwd,
                expected_version: read_response.save_config_version,
                create,
            })
            .await
        {
            Ok(response) => {
                let action = if create { "Created" } else { "Saved" };
                self.chat_widget.add_plain_history_lines(vec![
                    vec![
                        format!("{action} agent role ").green(),
                        response.role.name.clone().cyan(),
                        format!(" to {} scope.", role_scope_label(scope)).green(),
                    ]
                    .into(),
                    format!("Role file: {}", response.config_file).dim().into(),
                    format!(
                        "Config file: {}",
                        response.config_write.file_path.as_path().display()
                    )
                    .dim()
                    .into(),
                ]);
                if response.config_write.status == WriteStatus::OkOverridden {
                    let message = response
                        .config_write
                        .overridden_metadata
                        .map(|metadata| metadata.message)
                        .unwrap_or_else(|| {
                            "A higher-precedence config layer still overrides this role."
                                .to_string()
                        });
                    self.chat_widget.add_info_message(
                        "Agent role was saved but is currently overridden.".to_string(),
                        Some(message),
                    );
                }
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to save agent role `{name}`: {err}"));
            }
        }
    }
}

fn agent_roles_list_lines(response: &AgentRoleListResponse) -> Vec<Line<'static>> {
    let mut lines = vec!["Agent roles".bold().into()];
    if response.roles.is_empty() {
        lines.push("No agent roles available.".dim().into());
        return lines;
    }

    for role in &response.roles {
        let source = role_source_label(&role.source);
        let override_note = if role.overrides_built_in {
            " overrides built-in"
        } else {
            ""
        };
        lines.push(
            vec![
                "  ".into(),
                role.name.clone().cyan(),
                format!(" [{source}{override_note}]").dim(),
            ]
            .into(),
        );
        if let Some(description) = role.description.as_deref() {
            if let Some(first_line) = description.lines().next() {
                lines.push(vec!["    ".into(), first_line.to_string().dim()].into());
            }
        }
    }
    lines.push("Use /agent roles <role-name> for details.".dim().into());
    lines
}

fn agent_role_detail_lines(role: &AgentRoleSummary) -> Vec<Line<'static>> {
    let mut lines = vec![vec!["Agent role ".bold(), role.name.clone().cyan()].into()];
    lines.push(format!("Source: {}", role_source_label(&role.source)).into());
    if role.overrides_built_in {
        lines.push("Overrides built-in role: yes".into());
    }
    if let Some(config_file) = role.config_file.as_deref() {
        lines.push(format!("Config file: {config_file}").into());
    }
    if let Some(nicknames) = role.nickname_candidates.as_ref() {
        lines.push(format!("Nickname candidates: {}", nicknames.join(", ")).into());
    }
    if let Some(description) = role.description.as_deref() {
        lines.push("".into());
        lines.push("Description".bold().into());
        for line in description.lines() {
            lines.push(line.to_string().into());
        }
    }
    lines
}

fn role_source_label(source: &AgentRoleSource) -> &'static str {
    match source {
        AgentRoleSource::BuiltIn => "built-in",
        AgentRoleSource::Configured => "configured",
    }
}

fn role_scope_label(scope: AgentRoleSaveScope) -> &'static str {
    match scope {
        AgentRoleSaveScope::Global => "global",
        AgentRoleSaveScope::Local => "local",
    }
}
