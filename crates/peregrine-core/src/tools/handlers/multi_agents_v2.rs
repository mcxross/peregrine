//! Implements the MultiAgentV2 collaboration tool surface.

use crate::agent::AgentStatus;
use crate::agent::agent_resolver::resolve_agent_target;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::multi_agents_common::*;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use peregrine_types::AgentPath;
use peregrine_types::models::ResponseInputItem;
use peregrine_types::openai_models::ReasoningEffort;
use peregrine_types::protocol::CollabAgentInteractionBeginEvent;
use peregrine_types::protocol::CollabAgentInteractionEndEvent;
use peregrine_types::protocol::CollabAgentSpawnBeginEvent;
use peregrine_types::protocol::CollabAgentSpawnEndEvent;
use peregrine_types::protocol::CollabCloseBeginEvent;
use peregrine_types::protocol::CollabCloseEndEvent;
use peregrine_types::protocol::CollabWaitingBeginEvent;
use peregrine_types::protocol::CollabWaitingEndEvent;
use peregrine_types::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

pub(crate) use assign_task::Handler as AssignTaskHandler;
pub(crate) use close_agent::Handler as CloseAgentHandler;
pub(crate) use list_agents::Handler as ListAgentsHandler;
pub(crate) use send_message::Handler as SendMessageHandler;
pub(crate) use spawn::Handler as SpawnAgentHandler;
pub(crate) use wait::Handler as WaitAgentHandler;

mod assign_task;
mod close_agent;
mod list_agents;
mod message_tool;
mod send_message;
mod spawn;
pub(crate) mod wait;
