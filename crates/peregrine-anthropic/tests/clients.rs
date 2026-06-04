use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use peregrine_anthropic::AnthropicMessagesApiRequest;
use peregrine_anthropic::AnthropicMessagesClient;
use peregrine_anthropic::AnthropicMessagesOptions;
use peregrine_api::AuthProvider;
use peregrine_api::Provider;
use peregrine_api::ResponseEvent;
use peregrine_api::ResponsesApiRequest;
use peregrine_client::HttpTransport;
use peregrine_client::Request;
use peregrine_client::Response;
use peregrine_client::StreamResponse;
use peregrine_client::TransportError;
use peregrine_types::models::ContentItem;
use peregrine_types::models::ResponseItem;
use pretty_assertions::assert_eq;

fn assert_path_ends_with(requests: &[Request], suffix: &str) {
    assert_eq!(requests.len(), 1);
    let url = &requests[0].url;
    assert!(
        url.ends_with(suffix),
        "expected url to end with {suffix}, got {url}"
    );
}

#[derive(Debug, Default, Clone)]
struct RecordingState {
    stream_requests: Arc<Mutex<Vec<Request>>>,
}

impl RecordingState {
    fn record(&self, req: Request) {
        let mut guard = self
            .stream_requests
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"));
        guard.push(req);
    }

    fn take_stream_requests(&self) -> Vec<Request> {
        let mut guard = self
            .stream_requests
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"));
        std::mem::take(&mut *guard)
    }
}

#[derive(Clone)]
struct RecordingTransport {
    state: RecordingState,
}

impl RecordingTransport {
    fn new(state: RecordingState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl HttpTransport for RecordingTransport {
    async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Build("execute should not run".to_string()))
    }

    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
        self.state.record(req);

        let stream = futures::stream::iter(Vec::<Result<Bytes, TransportError>>::new());
        Ok(StreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            bytes: Box::pin(stream),
        })
    }
}

#[derive(Clone)]
struct StaticSseTransport {
    body: &'static str,
}

#[async_trait]
impl HttpTransport for StaticSseTransport {
    async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Build("execute should not run".to_string()))
    }

    async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
        let stream = futures::stream::iter(vec![Ok(Bytes::from_static(self.body.as_bytes()))]);
        Ok(StreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            bytes: Box::pin(stream),
        })
    }
}

#[derive(Debug, Clone)]
struct NoAuth;

impl AuthProvider for NoAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
}

fn anthropic_provider() -> Provider {
    let mut headers = HeaderMap::new();
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert("x-api-key", HeaderValue::from_static("test-anthropic-key"));

    Provider {
        name: "anthropic".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        query_params: None,
        headers,
        retry: peregrine_api::RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: true,
        },
        stream_idle_timeout: Duration::from_millis(10),
    }
}

#[tokio::test]
async fn anthropic_messages_client_uses_native_messages_path_and_headers() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let client = AnthropicMessagesClient::new(transport, anthropic_provider(), Arc::new(NoAuth));

    let request = AnthropicMessagesApiRequest::from_responses_request(ResponsesApiRequest {
        model: "claude-sonnet-4-6".into(),
        instructions: "You are concise.".into(),
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".into(),
            content: vec![ContentItem::InputText {
                text: "Say hi".into(),
            }],
            phase: None,
        }],
        tools: Vec::new(),
        tool_choice: "auto".into(),
        parallel_tool_calls: false,
        reasoning: None,
        store: false,
        stream: true,
        include: Vec::new(),
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    });

    let _stream = client
        .stream_request(
            request,
            AnthropicMessagesOptions {
                session_id: Some("sess_123".into()),
                thread_id: Some("thread_123".into()),
                ..Default::default()
            },
        )
        .await?;

    let requests = state.take_stream_requests();
    assert_path_ends_with(&requests, "/messages");
    let req = &requests[0];
    assert_eq!(
        req.headers
            .get(http::header::ACCEPT)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    assert_eq!(
        req.headers
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok()),
        Some("2023-06-01")
    );
    assert_eq!(
        req.headers.get("x-api-key").and_then(|v| v.to_str().ok()),
        Some("test-anthropic-key")
    );
    assert!(req.headers.get(http::header::AUTHORIZATION).is_none());
    Ok(())
}

#[test]
fn anthropic_messages_request_converts_messages_and_tools() {
    let request = AnthropicMessagesApiRequest::from_responses_request(ResponsesApiRequest {
        model: "claude-sonnet-4-6".into(),
        instructions: "Follow instructions.".into(),
        input: vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Run the tool".into(),
                }],
                phase: None,
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "echo".into(),
                namespace: None,
                arguments: r#"{"value":"hi"}"#.into(),
                call_id: "call_1".into(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_1".into(),
                output: peregrine_types::models::FunctionCallOutputPayload::from_text("ok".into()),
            },
        ],
        tools: vec![
            serde_json::json!({
                "type": "function",
                "name": "echo",
                "description": "Echo input.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "value": { "type": "string" }
                    }
                }
            }),
            serde_json::json!({
                "type": "web_search"
            }),
        ],
        tool_choice: "auto".into(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        include: Vec::new(),
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    });

    let body = serde_json::to_value(request).expect("serialize request");
    assert_eq!(body["model"], "claude-sonnet-4-6");
    assert_eq!(body["system"], "Follow instructions.");
    assert_eq!(body["max_tokens"], 4096);
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert_eq!(body["messages"][0]["content"][0]["text"], "Run the tool");
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(body["messages"][1]["content"][0]["id"], "call_1");
    assert_eq!(body["messages"][1]["content"][0]["name"], "echo");
    assert_eq!(body["messages"][1]["content"][0]["input"]["value"], "hi");
    assert_eq!(body["messages"][2]["role"], "user");
    assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "call_1");
    assert_eq!(body["messages"][2]["content"][0]["content"], "ok");
    assert_eq!(body["tools"].as_array().expect("tools").len(), 1);
    assert_eq!(body["tools"][0]["name"], "echo");
    assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    assert_eq!(body["tool_choice"]["type"], "auto");
}

#[test]
fn anthropic_messages_request_groups_adjacent_role_blocks() {
    let request = AnthropicMessagesApiRequest::from_responses_request(ResponsesApiRequest {
        model: "claude-sonnet-4-6".into(),
        instructions: String::new(),
        input: vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Check two things".into(),
                }],
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".into(),
                content: vec![ContentItem::OutputText {
                    text: "I will inspect both.".into(),
                }],
                phase: None,
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "read_file".into(),
                namespace: None,
                arguments: r#"{"path":"Cargo.toml"}"#.into(),
                call_id: "call_read".into(),
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "list_files".into(),
                namespace: None,
                arguments: r#"{"root":"crates"}"#.into(),
                call_id: "call_list".into(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_read".into(),
                output: peregrine_types::models::FunctionCallOutputPayload::from_text(
                    "workspace".into(),
                ),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_list".into(),
                output: peregrine_types::models::FunctionCallOutputPayload::from_text(
                    "peregrine-api".into(),
                ),
            },
        ],
        tools: Vec::new(),
        tool_choice: "auto".into(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        include: Vec::new(),
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    });

    let body = serde_json::to_value(request).expect("serialize request");
    assert_eq!(body["messages"].as_array().expect("messages").len(), 3);
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(
        body["messages"][1]["content"]
            .as_array()
            .expect("assistant content")
            .len(),
        3
    );
    assert_eq!(body["messages"][1]["content"][0]["type"], "text");
    assert_eq!(body["messages"][1]["content"][1]["type"], "tool_use");
    assert_eq!(body["messages"][1]["content"][2]["type"], "tool_use");
    assert_eq!(body["messages"][2]["role"], "user");
    assert_eq!(
        body["messages"][2]["content"]
            .as_array()
            .expect("tool result content")
            .len(),
        2
    );
    assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(body["messages"][2]["content"][1]["type"], "tool_result");
}

#[tokio::test]
async fn anthropic_messages_stream_maps_text_tool_call_and_completion() -> Result<()> {
    let body = r#"data: {"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":3,"output_tokens":1}}}

data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}

data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call_1","name":"echo","input":{}}}

data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"value\""}}

data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":":\"hi\"}"}}

data: {"type":"content_block_stop","index":1}

data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":5}}

data: {"type":"message_stop"}

"#;
    let transport = StaticSseTransport { body };
    let client = AnthropicMessagesClient::new(transport, anthropic_provider(), Arc::new(NoAuth));
    let request = AnthropicMessagesApiRequest::from_responses_request(ResponsesApiRequest {
        model: "claude-sonnet-4-6".into(),
        instructions: String::new(),
        input: Vec::new(),
        tools: Vec::new(),
        tool_choice: "auto".into(),
        parallel_tool_calls: false,
        reasoning: None,
        store: false,
        stream: true,
        include: Vec::new(),
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    });

    let mut stream = client
        .stream_request(request, AnthropicMessagesOptions::default())
        .await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event?;
        let is_completed = matches!(event, ResponseEvent::Completed { .. });
        events.push(event);
        if is_completed {
            break;
        }
    }

    assert!(matches!(events.first(), Some(ResponseEvent::Created)));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ResponseEvent::OutputTextDelta(delta) if delta == "hi"))
    );
    assert!(events.iter().any(|event| {
        matches!(
            event,
            ResponseEvent::OutputItemDone(ResponseItem::Message { role, content, .. })
                if role == "assistant"
                    && matches!(content.as_slice(), [ContentItem::OutputText { text }] if text == "hi")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            }) if name == "echo" && arguments == r#"{"value":"hi"}"# && call_id == "call_1"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            ResponseEvent::Completed {
                response_id,
                token_usage: Some(token_usage),
                end_turn: Some(false),
            } if response_id == "msg_1" && token_usage.total_tokens == 8
        )
    }));
    Ok(())
}
