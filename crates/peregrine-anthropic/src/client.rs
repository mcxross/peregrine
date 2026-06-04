use crate::request::AnthropicMessagesApiRequest;
use crate::sse::spawn_anthropic_messages_stream;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use peregrine_api::ApiError;
use peregrine_api::EndpointSession;
use peregrine_api::Provider;
use peregrine_api::RequestTelemetry;
use peregrine_api::ResponseStream;
use peregrine_api::SharedAuthProvider;
use peregrine_api::SseTelemetry;
use peregrine_api::build_session_headers;
use peregrine_api::insert_header;
use peregrine_api::subagent_header;
use peregrine_client::HttpTransport;
use peregrine_types::protocol::SessionSource;
use std::sync::Arc;
use tracing::instrument;

pub struct AnthropicMessagesClient<T: HttpTransport> {
    session: EndpointSession<T>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

#[derive(Default)]
pub struct AnthropicMessagesOptions {
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub session_source: Option<SessionSource>,
    pub extra_headers: HeaderMap,
}

impl<T: HttpTransport> AnthropicMessagesClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        Self {
            session: self.session.with_request_telemetry(request),
            sse_telemetry: sse,
        }
    }

    #[instrument(
        name = "anthropic_messages.stream_request",
        level = "info",
        skip_all,
        fields(
            transport = "anthropic_messages_http",
            http.method = "POST",
            api.path = "messages"
        )
    )]
    pub async fn stream_request(
        &self,
        request: AnthropicMessagesApiRequest,
        options: AnthropicMessagesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let AnthropicMessagesOptions {
            session_id,
            thread_id,
            session_source,
            mut extra_headers,
        } = options;

        let body = serde_json::to_value(&request).map_err(|error| {
            ApiError::Stream(format!(
                "failed to encode Anthropic Messages request: {error}"
            ))
        })?;
        if let Some(ref thread_id) = thread_id {
            insert_header(&mut extra_headers, "x-client-request-id", thread_id);
        }
        extra_headers.extend(build_session_headers(session_id, thread_id));
        if let Some(subagent) = subagent_header(&session_source) {
            insert_header(&mut extra_headers, "x-openai-subagent", &subagent);
        }

        let stream_response = self
            .session
            .stream_with(
                Method::POST,
                Self::path(),
                extra_headers,
                Some(body),
                |req| {
                    req.headers.insert(
                        http::header::ACCEPT,
                        HeaderValue::from_static("text/event-stream"),
                    );
                },
            )
            .await?;

        Ok(spawn_anthropic_messages_stream(
            stream_response,
            self.session.provider().stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }

    fn path() -> &'static str {
        "messages"
    }
}
