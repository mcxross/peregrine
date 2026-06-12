use super::*;
use crate::agent::history_cell;

impl ChatController {
    pub(super) fn fetch_mcp_inventory(
        &mut self,
        detail: McpServerStatusDetail,
        thread_id: Option<ThreadId>,
    ) -> ChatAction {
        if self.send_worker(WorkerCommand::FetchMcpInventory { detail, thread_id }) {
            return ChatAction::None;
        }

        if let Some(chat) = self.chat_widget.as_mut() {
            chat.clear_mcp_inventory_loading();
            chat.add_error_message(
                "Failed to load MCP inventory: chat worker is not ready.".to_string(),
            );
        }
        ChatAction::None
    }

    pub(super) fn apply_mcp_inventory_result(
        &mut self,
        result: std::result::Result<Vec<McpServerStatus>, String>,
        detail: McpServerStatusDetail,
        thread_id: Option<ThreadId>,
    ) {
        if thread_id.is_some() && thread_id != self.active_thread_id {
            return;
        }

        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        chat.clear_mcp_inventory_loading();

        let statuses = match result {
            Ok(statuses) => statuses,
            Err(err) => {
                chat.add_error_message(format!("Failed to load MCP inventory: {err}"));
                return;
            }
        };

        if statuses.is_empty() {
            chat.add_to_history(history_cell::empty_mcp_output());
        } else {
            chat.add_to_history(history_cell::new_mcp_tools_output_from_statuses(
                &statuses, detail,
            ));
        }
    }
}
