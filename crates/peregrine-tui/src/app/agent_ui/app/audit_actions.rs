use super::App;
use crate::agent::app_server_session::AppServerSession;
use crate::agent::audit_command::AuditCommand;
use crate::agent::audit_command::execute_audit_command;
use ratatui::style::Stylize;

impl App {
    pub(super) async fn handle_audit_command(
        &mut self,
        app_server: &mut AppServerSession,
        command: AuditCommand,
        command_text: String,
    ) {
        self.chat_widget
            .add_plain_history_lines(vec![command_text.magenta().into()]);
        match execute_audit_command(app_server, command).await {
            Ok(output) => self.chat_widget.add_plain_history_lines(output.lines),
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Audit command failed: {err}")),
        }
    }
}
