use crate::error::ApiError;
use futures::Stream;
use peregrine_types::config_types::ReasoningSummary as ReasoningSummaryConfig;
use peregrine_types::config_types::Verbosity as VerbosityConfig;
use peregrine_types::models::ResponseItem;
use peregrine_types::openai_models::ReasoningEffort as ReasoningEffortConfig;
use peregrine_types::protocol::ModelVerification;
use peregrine_types::protocol::RateLimitSnapshot;
use peregrine_types::protocol::TokenUsage;
use peregrine_types::protocol::W3cTraceContext;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

pub const WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY: &str = "ws_request_header_traceparent";
pub const WS_REQUEST_HEADER_TRACESTATE_CLIENT_METADATA_KEY: &str = "ws_request_header_tracestate";

/// Canonical input payload for the compaction endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct CompactionInput<'a> {
    pub model: &'a str,
    pub input: &'a [ResponseItem],
    #[serde(skip_serializing_if = "str::is_empty")]
    pub instructions: &'a str,
    pub tools: Vec<Value>,
    pub parallel_tool_calls: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextControls>,
}

/// Canonical input payload for the memory summarize endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct MemorySummarizeInput {
    pub model: String,
    #[serde(rename = "traces")]
    pub raw_memories: Vec<RawMemory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawMemory {
    pub id: String,
    pub metadata: RawMemoryMetadata,
    pub items: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawMemoryMetadata {
    pub source_path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MemorySummarizeOutput {
    #[serde(rename = "trace_summary", alias = "raw_memory")]
    pub raw_memory: String,
    pub memory_summary: String,
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    OutputItemAdded(ResponseItem),
    /// Emitted when the server includes `OpenAI-Model` on the stream response.
    /// This can differ from the requested model when backend safety routing applies.
    ServerModel(String),
    /// Emitted when the server recommends additional account verification.
    ModelVerifications(Vec<ModelVerification>),
    /// Emitted when `X-Reasoning-Included: true` is present on the response,
    /// meaning the server already accounted for past reasoning tokens and the
    /// client should not re-estimate them.
    ServerReasoningIncluded(bool),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
        /// Did the model affirmatively end its turn? Some providers do not set this,
        /// so we rely on fallback logic when this is `None`.
        end_turn: Option<bool>,
    },
    OutputTextDelta(String),
    ToolCallInputDelta {
        item_id: String,
        call_id: Option<String>,
        delta: String,
    },
    ReasoningSummaryDelta {
        delta: String,
        summary_index: i64,
    },
    ReasoningContentDelta {
        delta: String,
        content_index: i64,
    },
    ReasoningSummaryPartAdded {
        summary_index: i64,
    },
    RateLimits(RateLimitSnapshot),
    ModelsEtag(String),
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Reasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ReasoningSummaryConfig>,
}

#[derive(Debug, Serialize, Default, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TextFormatType {
    #[default]
    JsonSchema,
}

#[derive(Debug, Serialize, Default, Clone, PartialEq)]
pub struct TextFormat {
    /// Format type used by the OpenAI text controls.
    pub r#type: TextFormatType,
    /// When true, the server is expected to strictly validate responses.
    pub strict: bool,
    /// JSON schema for the desired output.
    pub schema: Value,
    /// Friendly name for the format, used in telemetry/debugging.
    pub name: String,
}

/// Controls the `text` field for the Responses API, combining verbosity and
/// optional JSON schema output formatting.
#[derive(Debug, Serialize, Default, Clone, PartialEq)]
pub struct TextControls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<OpenAiVerbosity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<TextFormat>,
}

#[derive(Debug, Serialize, Default, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OpenAiVerbosity {
    Low,
    #[default]
    Medium,
    High,
}

impl From<VerbosityConfig> for OpenAiVerbosity {
    fn from(v: VerbosityConfig) -> Self {
        match v {
            VerbosityConfig::Low => OpenAiVerbosity::Low,
            VerbosityConfig::Medium => OpenAiVerbosity::Medium,
            VerbosityConfig::High => OpenAiVerbosity::High,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ResponsesApiRequest {
    pub model: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub instructions: String,
    pub input: Vec<ResponseItem>,
    pub tools: Vec<serde_json::Value>,
    pub tool_choice: String,
    pub parallel_tool_calls: bool,
    pub reasoning: Option<Reasoning>,
    pub store: bool,
    pub stream: bool,
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextControls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ChatCompletionsApiRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatCompletionsStreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

impl ChatCompletionsApiRequest {
    pub fn from_responses_request(request: ResponsesApiRequest) -> Self {
        let ResponsesApiRequest {
            model,
            instructions,
            input,
            tools,
            tool_choice,
            parallel_tool_calls,
            ..
        } = request;

        let mut messages = Vec::new();
        if !instructions.trim().is_empty() {
            messages.push(ChatCompletionMessage::system(instructions));
        }
        messages.extend(response_items_to_chat_messages(input));

        let tools = responses_tools_to_chat_tools(tools);
        let tool_choice = if tools.is_empty() {
            None
        } else {
            Some(tool_choice)
        };
        let parallel_tool_calls = if tools.is_empty() {
            None
        } else {
            Some(parallel_tool_calls)
        };

        Self {
            model,
            messages,
            tools,
            tool_choice,
            parallel_tool_calls,
            stream: true,
            stream_options: Some(ChatCompletionsStreamOptions {
                include_usage: true,
            }),
            max_tokens: Some(4096),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ChatCompletionMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ChatCompletionToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatCompletionMessage {
    fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ChatCompletionToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatCompletionToolCallFunction,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ChatCompletionToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ChatCompletionsStreamOptions {
    pub include_usage: bool,
}

fn response_items_to_chat_messages(items: Vec<ResponseItem>) -> Vec<ChatCompletionMessage> {
    items
        .into_iter()
        .filter_map(response_item_to_chat_message)
        .collect()
}

fn response_item_to_chat_message(item: ResponseItem) -> Option<ChatCompletionMessage> {
    match item {
        ResponseItem::Message { role, content, .. } => {
            let content = content_items_to_text(content);
            if content.is_empty() {
                None
            } else {
                Some(ChatCompletionMessage {
                    role,
                    content: Some(content),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                })
            }
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => Some(ChatCompletionMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: vec![ChatCompletionToolCall {
                id: call_id,
                kind: "function".to_string(),
                function: ChatCompletionToolCallFunction { name, arguments },
            }],
            tool_call_id: None,
        }),
        ResponseItem::CustomToolCall {
            name,
            input,
            call_id,
            ..
        } => Some(ChatCompletionMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: vec![ChatCompletionToolCall {
                id: call_id,
                kind: "function".to_string(),
                function: ChatCompletionToolCallFunction {
                    name,
                    arguments: input,
                },
            }],
            tool_call_id: None,
        }),
        ResponseItem::FunctionCallOutput { call_id, output } => Some(ChatCompletionMessage {
            role: "tool".to_string(),
            content: Some(output.to_string()),
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id),
        }),
        ResponseItem::CustomToolCallOutput {
            call_id, output, ..
        } => Some(ChatCompletionMessage {
            role: "tool".to_string(),
            content: Some(output.to_string()),
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id),
        }),
        _ => None,
    }
}

fn content_items_to_text(items: Vec<peregrine_types::models::ContentItem>) -> String {
    items
        .into_iter()
        .filter_map(|item| match item {
            peregrine_types::models::ContentItem::InputText { text }
            | peregrine_types::models::ContentItem::OutputText { text } => Some(text),
            peregrine_types::models::ContentItem::InputImage { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn responses_tools_to_chat_tools(tools: Vec<Value>) -> Vec<Value> {
    tools
        .into_iter()
        .filter_map(response_tool_to_chat_tool)
        .collect()
}

fn response_tool_to_chat_tool(tool: Value) -> Option<Value> {
    let object = tool.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some("function") {
        return None;
    }
    let name = object.get("name")?.clone();
    let mut function = serde_json::Map::new();
    function.insert("name".to_string(), name);
    if let Some(description) = object.get("description") {
        function.insert("description".to_string(), description.clone());
    }
    if let Some(parameters) = object.get("parameters") {
        function.insert("parameters".to_string(), parameters.clone());
    }
    if let Some(strict) = object.get("strict") {
        function.insert("strict".to_string(), strict.clone());
    }

    Some(serde_json::json!({
        "type": "function",
        "function": Value::Object(function),
    }))
}

impl From<&ResponsesApiRequest> for ResponseCreateWsRequest {
    fn from(request: &ResponsesApiRequest) -> Self {
        Self {
            model: request.model.clone(),
            instructions: request.instructions.clone(),
            previous_response_id: None,
            input: request.input.clone(),
            tools: request.tools.clone(),
            tool_choice: request.tool_choice.clone(),
            parallel_tool_calls: request.parallel_tool_calls,
            reasoning: request.reasoning.clone(),
            store: request.store,
            stream: request.stream,
            include: request.include.clone(),
            service_tier: request.service_tier.clone(),
            prompt_cache_key: request.prompt_cache_key.clone(),
            text: request.text.clone(),
            generate: None,
            client_metadata: request.client_metadata.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ResponseCreateWsRequest {
    pub model: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub instructions: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    pub input: Vec<ResponseItem>,
    pub tools: Vec<Value>,
    pub tool_choice: String,
    pub parallel_tool_calls: bool,
    pub reasoning: Option<Reasoning>,
    pub store: bool,
    pub stream: bool,
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextControls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ResponseProcessedWsRequest {
    pub response_id: String,
}

pub fn response_create_client_metadata(
    client_metadata: Option<HashMap<String, String>>,
    trace: Option<&W3cTraceContext>,
) -> Option<HashMap<String, String>> {
    let mut client_metadata = client_metadata.unwrap_or_default();

    if let Some(traceparent) = trace.and_then(|trace| trace.traceparent.as_deref()) {
        client_metadata.insert(
            WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY.to_string(),
            traceparent.to_string(),
        );
    }
    if let Some(tracestate) = trace.and_then(|trace| trace.tracestate.as_deref()) {
        client_metadata.insert(
            WS_REQUEST_HEADER_TRACESTATE_CLIENT_METADATA_KEY.to_string(),
            tracestate.to_string(),
        );
    }

    (!client_metadata.is_empty()).then_some(client_metadata)
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum ResponsesWsRequest {
    #[serde(rename = "response.create")]
    ResponseCreate(ResponseCreateWsRequest),
    #[serde(rename = "response.processed")]
    ResponseProcessed(ResponseProcessedWsRequest),
}

pub fn create_text_param_for_request(
    verbosity: Option<VerbosityConfig>,
    output_schema: &Option<Value>,
    output_schema_strict: bool,
) -> Option<TextControls> {
    if verbosity.is_none() && output_schema.is_none() {
        return None;
    }

    Some(TextControls {
        verbosity: verbosity.map(std::convert::Into::into),
        format: output_schema.as_ref().map(|schema| TextFormat {
            r#type: TextFormatType::JsonSchema,
            strict: output_schema_strict,
            schema: schema.clone(),
            name: "peregrine_output_schema".to_string(),
        }),
    })
}

pub struct ResponseStream {
    pub rx_event: mpsc::Receiver<Result<ResponseEvent, ApiError>>,
    /// Server-assigned `x-request-id` response header, when present.
    pub upstream_request_id: Option<String>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent, ApiError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}
