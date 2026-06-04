use eventsource_stream::Eventsource;
use futures::StreamExt;
use peregrine_api::ApiError;
use peregrine_api::ResponseEvent;
use peregrine_api::ResponseStream;
use peregrine_api::SseTelemetry;
use peregrine_api::telemetry_enabled;
use peregrine_client::ByteStream;
use peregrine_client::StreamResponse;
use peregrine_types::models::ContentItem;
use peregrine_types::models::ResponseItem;
use peregrine_types::protocol::TokenUsage;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;

const REQUEST_ID_HEADER: &str = "request-id";
const X_REQUEST_ID_HEADER: &str = "x-request-id";

pub fn spawn_anthropic_messages_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let upstream_request_id = stream_response
        .headers
        .get(REQUEST_ID_HEADER)
        .or_else(|| stream_response.headers.get(X_REQUEST_ID_HEADER))
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_anthropic_messages_sse(stream_response.bytes, tx_event, idle_timeout, telemetry)
            .await;
    });

    ResponseStream {
        rx_event,
        upstream_request_id,
    }
}

#[derive(Debug, Default)]
struct AnthropicStreamState {
    response_id: Option<String>,
    input_tokens: i64,
    output_tokens: i64,
    stop_reason: Option<String>,
    text: String,
    tool_blocks: BTreeMap<i64, AnthropicToolUseBlock>,
}

#[derive(Debug, Default)]
struct AnthropicToolUseBlock {
    id: Option<String>,
    name: Option<String>,
    partial_json: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    kind: String,
    message: Option<AnthropicMessageStart>,
    index: Option<i64>,
    content_block: Option<AnthropicContentBlockStart>,
    delta: Option<AnthropicDelta>,
    usage: Option<AnthropicUsage>,
    error: Option<AnthropicStreamError>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageStart {
    id: String,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlockStart {
    #[serde(rename = "type")]
    kind: String,
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
    partial_json: Option<String>,
    stop_reason: Option<String>,
    thinking: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamError {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

pub async fn process_anthropic_messages_sse(
    stream: ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) {
    let mut stream = stream.eventsource();
    let mut state = AnthropicStreamState::default();

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
                debug!("Anthropic Messages SSE error: {error:#}");
                let _ = tx_event
                    .send(Err(ApiError::Stream(error.to_string())))
                    .await;
                return;
            }
            Ok(None) => {
                finalize_anthropic_stream(state, &tx_event).await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        let event: AnthropicStreamEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(error) => {
                debug!(
                    "failed to parse Anthropic Messages SSE event: {error}, data: {}",
                    sse.data
                );
                continue;
            }
        };

        match event.kind.as_str() {
            "message_start" => {
                if let Some(message) = event.message {
                    state.response_id = Some(message.id);
                    if let Some(usage) = message.usage {
                        if let Some(input_tokens) = usage.input_tokens {
                            state.input_tokens = input_tokens;
                        }
                        if let Some(output_tokens) = usage.output_tokens {
                            state.output_tokens = output_tokens;
                        }
                    }
                }
                if tx_event.send(Ok(ResponseEvent::Created)).await.is_err() {
                    return;
                }
            }
            "content_block_start" => {
                if let (Some(index), Some(block)) = (event.index, event.content_block)
                    && block.kind == "tool_use"
                {
                    state.tool_blocks.insert(
                        index,
                        AnthropicToolUseBlock {
                            id: block.id,
                            name: block.name,
                            partial_json: String::new(),
                        },
                    );
                }
            }
            "content_block_delta" => {
                let Some(delta) = event.delta else {
                    continue;
                };
                match delta.kind.as_deref() {
                    Some("text_delta") => {
                        if let Some(text) = delta.text {
                            state.text.push_str(&text);
                            if tx_event
                                .send(Ok(ResponseEvent::OutputTextDelta(text)))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    Some("input_json_delta") => {
                        if let (Some(index), Some(partial_json)) = (event.index, delta.partial_json)
                            && let Some(block) = state.tool_blocks.get_mut(&index)
                        {
                            if let Some(id) = block.id.clone()
                                && tx_event
                                    .send(Ok(ResponseEvent::ToolCallInputDelta {
                                        item_id: id,
                                        call_id: block.id.clone(),
                                        delta: partial_json.clone(),
                                    }))
                                    .await
                                    .is_err()
                            {
                                return;
                            }
                            block.partial_json.push_str(&partial_json);
                        }
                    }
                    Some("thinking_delta") => {
                        if let (Some(index), Some(thinking)) = (event.index, delta.thinking)
                            && tx_event
                                .send(Ok(ResponseEvent::ReasoningContentDelta {
                                    delta: thinking,
                                    content_index: index,
                                }))
                                .await
                                .is_err()
                        {
                            return;
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let Some(index) = event.index
                    && let Some(block) = state.tool_blocks.remove(&index)
                {
                    let Some(name) = block.name else {
                        continue;
                    };
                    let call_id = block.id.unwrap_or_else(|| format!("call_{name}"));
                    let arguments =
                        normalize_tool_arguments_for_responses_item(&block.partial_json);
                    if tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(
                            ResponseItem::FunctionCall {
                                id: None,
                                name,
                                namespace: None,
                                arguments,
                                call_id,
                            },
                        )))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
            "message_delta" => {
                if let Some(delta) = event.delta
                    && let Some(stop_reason) = delta.stop_reason
                {
                    state.stop_reason = Some(stop_reason);
                }
                if let Some(usage) = event.usage
                    && let Some(output_tokens) = usage.output_tokens
                {
                    state.output_tokens = output_tokens;
                }
            }
            "message_stop" => {
                finalize_anthropic_stream(state, &tx_event).await;
                return;
            }
            "error" => {
                let message = event
                    .error
                    .map(|error| format!("{}: {}", error.kind, error.message))
                    .unwrap_or_else(|| "Anthropic stream error".to_string());
                let _ = tx_event.send(Err(ApiError::Stream(message))).await;
                return;
            }
            _ => {}
        }
    }
}

fn normalize_tool_arguments_for_responses_item(partial_json: &str) -> String {
    match serde_json::from_str::<Value>(partial_json) {
        Ok(value) => value.to_string(),
        Err(_) => partial_json.to_string(),
    }
}

async fn finalize_anthropic_stream(
    state: AnthropicStreamState,
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
) {
    if !state.text.is_empty()
        && tx_event
            .send(Ok(ResponseEvent::OutputItemDone(ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText { text: state.text }],
                phase: None,
            })))
            .await
            .is_err()
    {
        return;
    }

    let end_turn = match state.stop_reason.as_deref() {
        Some("end_turn") => Some(true),
        Some("tool_use") => Some(false),
        _ => None,
    };
    let total_tokens = state.input_tokens.saturating_add(state.output_tokens);
    let token_usage = if total_tokens > 0 {
        Some(TokenUsage {
            input_tokens: state.input_tokens,
            cached_input_tokens: 0,
            output_tokens: state.output_tokens,
            reasoning_output_tokens: 0,
            total_tokens,
        })
    } else {
        None
    };
    let response_id = state
        .response_id
        .unwrap_or_else(|| "anthropic-message".to_string());
    let _ = tx_event
        .send(Ok(ResponseEvent::Completed {
            response_id,
            token_usage,
            end_turn,
        }))
        .await;
}
