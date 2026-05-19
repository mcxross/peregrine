use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::Emitter;

const OLLAMA_CHAT_STREAM_EVENT: &str = "ollama-chat-stream";
const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const OLLAMA_FALLBACK_BASE_URL: &str = "http://localhost:11434";
const OLLAMA_KEEP_ALIVE: &str = "30m";

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatApiResponse {
    message: Option<OllamaChatMessage>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatPayload {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    think: bool,
    #[serde(rename = "keep_alive")]
    keep_alive: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OllamaChatStreamEvent {
    stream_id: String,
    kind: &'static str,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatStreamResponse {
    done: Option<bool>,
    message: Option<OllamaChatMessage>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OllamaChatResponse {
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaGenerateApiResponse {
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaPreloadPayload {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(rename = "keep_alive")]
    keep_alive: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OllamaPreloadResponse {
    model: String,
    keep_alive: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaTagsApiResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OllamaModel {
    name: String,
}

struct OllamaStreamError {
    had_content: bool,
    message: String,
}

#[tauri::command]
pub(crate) async fn list_ollama_models(
    base_url: Option<String>,
) -> Result<Vec<OllamaModel>, String> {
    let client = ollama_client()?;
    let body = send_ollama_request(
        &client,
        "/api/tags",
        "model list",
        base_url.as_deref(),
        |client, url| client.get(url).header("Accept", "application/json"),
    )
    .await?;
    let tags: OllamaTagsApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Ollama model list: {error}"))?;

    Ok(tags.models)
}

#[tauri::command]
pub(crate) async fn chat_with_ollama(
    model: String,
    messages: Vec<OllamaChatMessage>,
) -> Result<OllamaChatResponse, String> {
    let payload = OllamaChatPayload {
        model: model.trim().to_string(),
        messages,
        stream: false,
        think: false,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    };
    let request_body = serde_json::to_string(&payload)
        .map_err(|error| format!("Could not serialize Ollama request: {error}"))?;
    let client = ollama_client()?;
    let body = send_ollama_request(&client, "/api/chat", "chat", None, |client, url| {
        client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(request_body.clone())
    })
    .await?;
    let chat: OllamaChatApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Ollama response: {error}"))?;

    if let Some(error) = chat.error {
        return Err(error);
    }

    Ok(OllamaChatResponse {
        content: chat
            .message
            .map(|message| message.content)
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub(crate) async fn preload_ollama_model(model: String) -> Result<OllamaPreloadResponse, String> {
    let model = model.trim().to_string();

    if model.is_empty() {
        return Err("Select an Ollama model before preloading.".to_string());
    }

    let payload = OllamaPreloadPayload {
        model: model.clone(),
        prompt: String::new(),
        stream: false,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    };
    let request_body = serde_json::to_string(&payload)
        .map_err(|error| format!("Could not serialize Ollama preload request: {error}"))?;
    let client = ollama_client()?;

    eprintln!("[peregrine:ollama] Preloading model={model} keep_alive={OLLAMA_KEEP_ALIVE}");

    let body = send_ollama_request(&client, "/api/generate", "preload", None, |client, url| {
        client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(request_body.clone())
    })
    .await?;
    let response: OllamaGenerateApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Ollama preload response: {error}"))?;

    if let Some(error) = response.error {
        return Err(error);
    }

    eprintln!("[peregrine:ollama] Preloaded model={model}");

    Ok(OllamaPreloadResponse {
        model,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    })
}

#[tauri::command]
pub(crate) async fn stream_chat_with_ollama(
    app: tauri::AppHandle,
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream_id: String,
) -> Result<OllamaChatResponse, String> {
    let payload = OllamaChatPayload {
        model: model.trim().to_string(),
        messages,
        stream: true,
        think: false,
        keep_alive: OLLAMA_KEEP_ALIVE.to_string(),
    };
    let request_body = serde_json::to_string(&payload)
        .map_err(|error| format!("Could not serialize Ollama stream request: {error}"))?;
    let client = ollama_client()?;
    let base_urls = [OLLAMA_BASE_URL, OLLAMA_FALLBACK_BASE_URL];
    let mut errors = Vec::new();

    emit_ollama_log(
        &app,
        &stream_id,
        format!(
            "Preparing Ollama stream request: endpoint=/api/chat model={} messages={} bytes={}",
            payload.model,
            payload.messages.len(),
            request_body.len()
        ),
    );

    for base_url in base_urls {
        match stream_single_ollama_chat(&client, base_url, &request_body, &app, &stream_id).await {
            Ok(content) => {
                emit_ollama_event(&app, &stream_id, "done", "");
                return Ok(OllamaChatResponse { content });
            }
            Err(error) => {
                emit_ollama_log(&app, &stream_id, error.message.clone());

                if error.had_content {
                    emit_ollama_event(&app, &stream_id, "error", &error.message);
                    return Err(error.message);
                }

                errors.push(error.message);
            }
        }
    }

    let error = format!(
        "Could not stream from local Ollama. Tried {}. {}",
        base_urls.join(", "),
        errors.join(" ")
    );
    emit_ollama_event(&app, &stream_id, "error", &error);
    Err(error)
}

fn ollama_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .http1_only()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(240))
        .build()
        .map_err(|error| format!("Could not create Ollama HTTP client: {error}"))
}

async fn send_ollama_request<F>(
    client: &reqwest::Client,
    path: &str,
    action: &str,
    preferred_base_url: Option<&str>,
    build_request: F,
) -> Result<String, String>
where
    F: Fn(&reqwest::Client, String) -> reqwest::RequestBuilder,
{
    let base_urls = ollama_base_urls(preferred_base_url);
    let mut errors = Vec::new();

    for base_url in &base_urls {
        let url = format!("{base_url}{path}");
        match send_single_ollama_request(build_request(client, url), action, base_url).await {
            Ok(body) => return Ok(body),
            Err(error) => errors.push(error),
        }
    }

    Err(format!(
        "Could not reach local Ollama while requesting {action}. Tried {}. {}",
        base_urls.join(", "),
        errors.join(" ")
    ))
}

fn ollama_base_urls(preferred_base_url: Option<&str>) -> Vec<String> {
    let mut urls = Vec::new();

    if let Some(url) = preferred_base_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(trim_trailing_slash)
    {
        urls.push(url);
    }

    for fallback in [OLLAMA_BASE_URL, OLLAMA_FALLBACK_BASE_URL] {
        let fallback = trim_trailing_slash(fallback);

        if !urls.iter().any(|url| url == &fallback) {
            urls.push(fallback);
        }
    }

    urls
}

fn trim_trailing_slash(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

async fn send_single_ollama_request(
    request: reqwest::RequestBuilder,
    action: &str,
    base_url: &str,
) -> Result<String, String> {
    let response = request.send().await.map_err(|error| {
        format!(
            "Request to {base_url} failed: {}",
            describe_reqwest_error(&error)
        )
    })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        format!(
            "Could not read Ollama {action} response from {base_url}: {}",
            describe_reqwest_error(&error)
        )
    })?;

    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| body.trim().to_string());

        return Err(if message.is_empty() {
            format!("Ollama at {base_url} returned HTTP {status} while requesting {action}.")
        } else {
            format!(
                "Ollama at {base_url} returned HTTP {status} while requesting {action}: {message}"
            )
        });
    }

    Ok(body)
}

async fn stream_single_ollama_chat(
    client: &reqwest::Client,
    base_url: &str,
    request_body: &str,
    app: &tauri::AppHandle,
    stream_id: &str,
) -> Result<String, OllamaStreamError> {
    let url = format!("{base_url}/api/chat");
    emit_ollama_log(app, stream_id, format!("Opening Ollama stream: {url}"));

    let mut response = client
        .post(url)
        .header("Accept", "application/x-ndjson")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .send()
        .await
        .map_err(|error| OllamaStreamError {
            had_content: false,
            message: format!(
                "Stream request to {base_url} failed: {}",
                describe_reqwest_error(&error)
            ),
        })?;
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.map_err(|error| OllamaStreamError {
            had_content: false,
            message: format!(
                "Could not read Ollama stream error response from {base_url}: {}",
                describe_reqwest_error(&error)
            ),
        })?;
        let message = parse_ollama_error_body(&body);

        return Err(OllamaStreamError {
            had_content: false,
            message: if message.is_empty() {
                format!("Ollama at {base_url} returned HTTP {status} for stream request.")
            } else {
                format!("Ollama at {base_url} returned HTTP {status} for stream request: {message}")
            },
        });
    }

    let mut pending = Vec::<u8>::new();
    let mut content = String::new();

    while let Some(chunk) = response.chunk().await.map_err(|error| OllamaStreamError {
        had_content: !content.is_empty(),
        message: format!(
            "Could not read Ollama stream chunk from {base_url}: {}",
            describe_reqwest_error(&error)
        ),
    })? {
        pending.extend_from_slice(&chunk);

        while let Some(line_end) = pending.iter().position(|byte| *byte == b'\n') {
            let line = pending.drain(..=line_end).collect::<Vec<_>>();
            process_ollama_stream_line(&line, &mut content, app, stream_id).map_err(|message| {
                OllamaStreamError {
                    had_content: !content.is_empty(),
                    message,
                }
            })?;
        }
    }

    if !pending.is_empty() {
        process_ollama_stream_line(&pending, &mut content, app, stream_id).map_err(|message| {
            OllamaStreamError {
                had_content: !content.is_empty(),
                message,
            }
        })?;
    }

    emit_ollama_log(
        app,
        stream_id,
        format!(
            "Ollama stream completed from {base_url}; response_bytes={}",
            content.len()
        ),
    );

    Ok(content)
}

fn process_ollama_stream_line(
    line: &[u8],
    content: &mut String,
    app: &tauri::AppHandle,
    stream_id: &str,
) -> Result<(), String> {
    let line = String::from_utf8_lossy(line).trim().to_string();

    if line.is_empty() {
        return Ok(());
    }

    let response: OllamaChatStreamResponse = serde_json::from_str(&line)
        .map_err(|error| format!("Could not parse Ollama stream line: {error}; line={line}"))?;

    if let Some(error) = response.error {
        return Err(error);
    }

    if let Some(message) = response.message {
        if !message.content.is_empty() {
            content.push_str(&message.content);
            emit_ollama_event(app, stream_id, "chunk", &message.content);
        }
    }

    if response.done.unwrap_or(false) {
        emit_ollama_log(app, stream_id, "Ollama stream reported done.".to_string());
    }

    Ok(())
}

fn parse_ollama_error_body(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.trim().to_string())
}

fn describe_reqwest_error(error: &reqwest::Error) -> String {
    let mut details = vec![error.to_string()];
    let mut source = std::error::Error::source(error);

    while let Some(next_source) = source {
        details.push(next_source.to_string());
        source = next_source.source();
    }

    details.join(": ")
}

fn emit_ollama_log(app: &tauri::AppHandle, stream_id: &str, message: String) {
    eprintln!("[peregrine:ollama] {message}");
    emit_ollama_event(app, stream_id, "debug", &message);
}

fn emit_ollama_event(app: &tauri::AppHandle, stream_id: &str, kind: &'static str, content: &str) {
    let _ = app.emit(
        OLLAMA_CHAT_STREAM_EVENT,
        OllamaChatStreamEvent {
            stream_id: stream_id.to_string(),
            kind,
            content: content.to_string(),
        },
    );
}
