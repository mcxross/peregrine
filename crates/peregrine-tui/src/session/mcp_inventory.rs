use color_eyre::eyre::{Result, WrapErr};
use peregrine_app_server_client::AppServerRequestHandle;
use peregrine_app_server_protocol::{
    ClientRequest, ListMcpServerStatusParams, ListMcpServerStatusResponse, McpServerStatus,
    McpServerStatusDetail, RequestId,
};
use peregrine_types::ThreadId;
use uuid::Uuid;

pub(crate) async fn fetch_all_mcp_server_statuses(
    request_handle: AppServerRequestHandle,
    detail: McpServerStatusDetail,
    thread_id: Option<ThreadId>,
) -> Result<Vec<McpServerStatus>> {
    let mut cursor = None;
    let mut statuses = Vec::new();
    let thread_id = thread_id.map(|id| id.to_string());

    loop {
        let request_id = RequestId::String(format!("mcp-inventory-{}", Uuid::new_v4()));
        let response: ListMcpServerStatusResponse = request_handle
            .request_typed(ClientRequest::McpServerStatusList {
                request_id,
                params: ListMcpServerStatusParams {
                    cursor: cursor.clone(),
                    limit: Some(100),
                    detail: Some(detail),
                    thread_id: thread_id.clone(),
                },
            })
            .await
            .wrap_err("mcpServerStatus/list failed in TUI")?;
        statuses.extend(response.data);
        if let Some(next_cursor) = response.next_cursor {
            cursor = Some(next_cursor);
        } else {
            break;
        }
    }

    Ok(statuses)
}
