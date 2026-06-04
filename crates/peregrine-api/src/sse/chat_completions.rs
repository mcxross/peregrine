use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use crate::telemetry::telemetry_enabled;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use peregrine_client::ByteStream;
use peregrine_client::StreamResponse;
use peregrine_types::models::ContentItem;
use peregrine_types::models::ResponseItem;
use peregrine_types::protocol::TokenUsage;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;

const REQUEST_ID_HEADER: &str = "x-request-id";

pub fn spawn_chat_completions_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let upstream_request_id = stream_response
        .headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_chat_completions_sse(stream_response.bytes, tx_event, idle_timeout, telemetry)
            .await;
    });

    ResponseStream {
        rx_event,
        upstream_request_id,
    }
}

#[derive(Debug, Default)]
struct ChatCompletionsAccumulator {
    response_id: Option<String>,
    created_sent: bool,
    content: String,
    tool_calls: BTreeMap<u64, AccumulatedToolCall>,
    token_usage: Option<TokenUsage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Default)]
struct AccumulatedToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    id: Option<String>,
    #[serde(default)]
    choices: Vec<ChatCompletionChoice>,
    usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    delta: ChatCompletionDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ChatCompletionToolCallDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionToolCallDelta {
    index: u64,
    id: Option<String>,
    function: Option<ChatCompletionFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionUsage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

impl From<ChatCompletionUsage> for TokenUsage {
    fn from(value: ChatCompletionUsage) -> Self {
        let input_tokens = value.prompt_tokens.unwrap_or_default();
        let output_tokens = value.completion_tokens.unwrap_or_default();
        TokenUsage {
            input_tokens,
            cached_input_tokens: 0,
            output_tokens,
            reasoning_output_tokens: 0,
            total_tokens: value
                .total_tokens
                .unwrap_or_else(|| input_tokens.saturating_add(output_tokens)),
        }
    }
}

pub async fn process_chat_completions_sse(
    stream: ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) {
    let mut stream = stream.eventsource();
    let mut state = ChatCompletionsAccumulator::default();

    loop {
        let start = Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        if telemetry_enabled()
            && let Some(t) = telemetry.as_ref()
        {
            t.on_sse_poll(&response, start.elapsed());
        }
        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(error))) => {
                debug!("Chat completions SSE error: {error:#}");
                let _ = tx_event
                    .send(Err(ApiError::Stream(error.to_string())))
                    .await;
                return;
            }
            Ok(None) => {
                finalize_chat_completions_stream(state, &tx_event).await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        if sse.data.trim() == "[DONE]" {
            finalize_chat_completions_stream(state, &tx_event).await;
            return;
        }

        let chunk: ChatCompletionChunk = match serde_json::from_str(&sse.data) {
            Ok(chunk) => chunk,
            Err(error) => {
                debug!(
                    "failed to parse chat completions SSE chunk: {error}, data: {}",
                    sse.data
                );
                continue;
            }
        };

        if let Some(id) = chunk.id {
            state.response_id.get_or_insert(id);
        }
        if let Some(usage) = chunk.usage {
            state.token_usage = Some(usage.into());
        }
        if !state.created_sent {
            state.created_sent = true;
            if tx_event.send(Ok(ResponseEvent::Created)).await.is_err() {
                return;
            }
        }

        for choice in chunk.choices {
            if let Some(content) = choice.delta.content {
                state.content.push_str(&content);
                if tx_event
                    .send(Ok(ResponseEvent::OutputTextDelta(content)))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            for tool_call in choice.delta.tool_calls {
                let entry = state.tool_calls.entry(tool_call.index).or_default();
                if let Some(id) = tool_call.id {
                    entry.id = Some(id);
                }
                if let Some(function) = tool_call.function {
                    if let Some(name) = function.name {
                        entry.name = Some(name);
                    }
                    if let Some(arguments) = function.arguments {
                        if let Some(id) = entry.id.clone()
                            && tx_event
                                .send(Ok(ResponseEvent::ToolCallInputDelta {
                                    item_id: id,
                                    call_id: entry.id.clone(),
                                    delta: arguments.clone(),
                                }))
                                .await
                                .is_err()
                        {
                            return;
                        }
                        entry.arguments.push_str(&arguments);
                    }
                }
            }
            if let Some(reason) = choice.finish_reason {
                state.finish_reason = Some(reason);
            }
        }
    }
}

async fn finalize_chat_completions_stream(
    state: ChatCompletionsAccumulator,
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
) {
    if !state.content.is_empty()
        && tx_event
            .send(Ok(ResponseEvent::OutputItemDone(ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: state.content,
                }],
                phase: None,
            })))
            .await
            .is_err()
    {
        return;
    }

    let has_tool_calls = !state.tool_calls.is_empty();
    for (_, tool_call) in state.tool_calls {
        let Some(name) = tool_call.name else {
            continue;
        };
        let call_id = tool_call.id.unwrap_or_else(|| format!("call_{name}"));
        if tx_event
            .send(Ok(ResponseEvent::OutputItemDone(
                ResponseItem::FunctionCall {
                    id: None,
                    name,
                    namespace: None,
                    arguments: tool_call.arguments,
                    call_id,
                },
            )))
            .await
            .is_err()
        {
            return;
        }
    }

    let end_turn = match state.finish_reason.as_deref() {
        Some("tool_calls") | Some("function_call") => Some(false),
        Some("stop") => Some(true),
        _ if has_tool_calls => Some(false),
        _ => None,
    };
    let response_id = state
        .response_id
        .unwrap_or_else(|| "chat-completions".to_string());
    let _ = tx_event
        .send(Ok(ResponseEvent::Completed {
            response_id,
            token_usage: state.token_usage,
            end_turn,
        }))
        .await;
}
