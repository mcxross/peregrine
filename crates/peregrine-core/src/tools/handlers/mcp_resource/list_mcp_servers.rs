use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::mcp_resource_spec::create_list_mcp_servers_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use peregrine_types::models::function_call_output_content_items_to_text;
use peregrine_types::protocol::McpInvocation;

use super::DEFAULT_MCP_INVENTORY_LIMIT;
use super::HOST_MCP_SERVER_LABEL;
use super::ListMcpServersArgs;
use super::ListMcpServersPayload;
use super::MAX_MCP_INVENTORY_LIMIT;
use super::call_tool_result_from_content;
use super::emit_tool_call_begin;
use super::emit_tool_call_end;
use super::normalize_optional_string;
use super::parse_args_with_default;
use super::parse_arguments;
use super::serialize_function_output;

pub struct ListMcpServersHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ListMcpServersHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("list_mcp_servers")
    }

    fn spec(&self) -> ToolSpec {
        create_list_mcp_servers_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    #[allow(
        clippy::await_holding_invalid_type,
        reason = "MCP tool inventory reads through the session-owned manager guard"
    )]
    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "list_mcp_servers handler received unsupported payload".to_string(),
                ));
            }
        };

        let arguments = parse_arguments(arguments.as_str())?;
        let args: ListMcpServersArgs = parse_args_with_default(arguments.clone())?;
        let server = normalize_optional_string(args.server);
        let cursor = normalize_optional_string(args.cursor);
        let start = match cursor.as_deref() {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| {
                FunctionCallError::RespondToModel(format!(
                    "invalid list_mcp_servers cursor: {cursor}"
                ))
            })?,
            None => 0,
        };
        let limit = args.limit.unwrap_or(DEFAULT_MCP_INVENTORY_LIMIT);
        if !(1..=MAX_MCP_INVENTORY_LIMIT).contains(&limit) {
            return Err(FunctionCallError::RespondToModel(format!(
                "limit must be between 1 and {MAX_MCP_INVENTORY_LIMIT}"
            )));
        }

        let invocation = McpInvocation {
            server: HOST_MCP_SERVER_LABEL.to_string(),
            tool: "list_mcp_servers".to_string(),
            arguments: arguments.clone(),
        };

        emit_tool_call_begin(&session, turn.as_ref(), &call_id, invocation.clone()).await;
        let start_time = Instant::now();

        let tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;
        let payload_result =
            ListMcpServersPayload::from_tools(tools, server.as_deref(), start, limit);

        match payload_result.and_then(serialize_function_output) {
            Ok(output) => {
                let content =
                    function_call_output_content_items_to_text(&output.body).unwrap_or_default();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    start_time.elapsed(),
                    Ok(call_tool_result_from_content(&content, output.success)),
                )
                .await;
                Ok(boxed_tool_output(output))
            }
            Err(err) => {
                let message = err.to_string();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    start_time.elapsed(),
                    Err(message),
                )
                .await;
                Err(err)
            }
        }
    }
}

impl CoreToolRuntime for ListMcpServersHandler {}
