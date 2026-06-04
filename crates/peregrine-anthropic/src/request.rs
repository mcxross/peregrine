use peregrine_api::ResponsesApiRequest;
use peregrine_types::models::ContentItem;
use peregrine_types::models::ResponseItem;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AnthropicMessagesApiRequest {
    pub model: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub system: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<AnthropicTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<AnthropicToolChoice>,
    pub stream: bool,
}

impl AnthropicMessagesApiRequest {
    pub fn from_responses_request(request: ResponsesApiRequest) -> Self {
        let ResponsesApiRequest {
            model,
            instructions,
            input,
            tools,
            ..
        } = request;
        let tools = responses_tools_to_anthropic_tools(tools);
        let tool_choice = if tools.is_empty() {
            None
        } else {
            Some(AnthropicToolChoice {
                r#type: "auto".to_string(),
            })
        };

        Self {
            model,
            max_tokens: 4096,
            system: instructions,
            messages: response_items_to_anthropic_messages(input),
            tools,
            tool_choice,
            stream: true,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AnthropicTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AnthropicToolChoice {
    #[serde(rename = "type")]
    pub r#type: String,
}

fn response_items_to_anthropic_messages(items: Vec<ResponseItem>) -> Vec<AnthropicMessage> {
    let mut messages: Vec<AnthropicMessage> = Vec::new();

    for item in items {
        let Some((role, content)) = response_item_to_anthropic_content(item) else {
            continue;
        };
        if let Some(last) = messages.last_mut()
            && last.role == role
        {
            last.content.extend(content);
            continue;
        }
        messages.push(AnthropicMessage { role, content });
    }

    messages
}

fn response_item_to_anthropic_content(
    item: ResponseItem,
) -> Option<(String, Vec<AnthropicContentBlock>)> {
    match item {
        ResponseItem::Message { role, content, .. } => {
            let text = content_items_to_text(content);
            if text.is_empty() || !matches!(role.as_str(), "user" | "assistant") {
                None
            } else {
                Some((role, vec![AnthropicContentBlock::Text { text }]))
            }
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => Some((
            "assistant".to_string(),
            vec![AnthropicContentBlock::ToolUse {
                id: call_id,
                name,
                input: parse_tool_arguments(arguments),
            }],
        )),
        ResponseItem::CustomToolCall {
            name,
            input,
            call_id,
            ..
        } => Some((
            "assistant".to_string(),
            vec![AnthropicContentBlock::ToolUse {
                id: call_id,
                name,
                input: parse_tool_arguments(input),
            }],
        )),
        ResponseItem::FunctionCallOutput { call_id, output } => Some((
            "user".to_string(),
            vec![AnthropicContentBlock::ToolResult {
                tool_use_id: call_id,
                content: output.to_string(),
            }],
        )),
        ResponseItem::CustomToolCallOutput {
            call_id, output, ..
        } => Some((
            "user".to_string(),
            vec![AnthropicContentBlock::ToolResult {
                tool_use_id: call_id,
                content: output.to_string(),
            }],
        )),
        _ => None,
    }
}

fn parse_tool_arguments(arguments: String) -> Value {
    serde_json::from_str(&arguments).unwrap_or_else(|_| {
        serde_json::json!({
            "input": arguments,
        })
    })
}

fn responses_tools_to_anthropic_tools(tools: Vec<Value>) -> Vec<AnthropicTool> {
    tools
        .into_iter()
        .filter_map(response_tool_to_anthropic_tool)
        .collect()
}

fn response_tool_to_anthropic_tool(tool: Value) -> Option<AnthropicTool> {
    let object = tool.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some("function") {
        return None;
    }
    let name = object.get("name")?.as_str()?.to_string();
    let description = object
        .get("description")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let input_schema = object
        .get("parameters")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} }));

    Some(AnthropicTool {
        name,
        description,
        input_schema,
    })
}

fn content_items_to_text(items: Vec<ContentItem>) -> String {
    items
        .into_iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => Some(text),
            ContentItem::InputImage { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
