use super::session::AgentServerSessions;
use peregrine_app_server_client::{AppServerClient, AppServerEvent};
use peregrine_app_server_protocol::{JSONRPCErrorError, ServerNotification, ServerRequest};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

pub(crate) const AGENT_SERVER_EVENT: &str = "agent-server-event";
pub(crate) const AGENT_SERVER_REQUEST: &str = "agent-server-request";
pub(crate) const AGENT_SERVER_DISCONNECTED: &str = "agent-server-disconnected";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentServerEventEnvelope {
    session_id: String,
    run_id: String,
    event: AgentServerEventPayload,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum AgentServerEventPayload {
    Lagged { skipped: usize },
    Notification { notification: ServerNotification },
    Disconnected { message: String },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentServerRequestEnvelope {
    session_id: String,
    run_id: String,
    request: ServerRequest,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentServerDisconnectedEnvelope {
    session_id: String,
    run_id: String,
    message: String,
}

pub(crate) fn spawn_event_pump(
    app: AppHandle,
    sessions: Arc<AgentServerSessions>,
    session_id: String,
    mut client: AppServerClient,
    mut stop_rx: oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    let _ = client.shutdown().await;
                    break;
                }
                event = client.next_event() => {
                    let Some(event) = event else {
                        emit_disconnected(&app, &session_id, "app-server event stream closed");
                        sessions.mark_disconnected(&session_id).await;
                        break;
                    };

                    match event {
                        AppServerEvent::Lagged { skipped } => {
                            emit_event(
                                &app,
                                &session_id,
                                AgentServerEventPayload::Lagged { skipped },
                            );
                        }
                        AppServerEvent::ServerNotification(notification) => {
                            sessions.apply_notification(&session_id, &notification).await;
                            emit_event(
                                &app,
                                &session_id,
                                AgentServerEventPayload::Notification { notification },
                            );
                        }
                        AppServerEvent::ServerRequest(request) => {
                            if should_reject_unsupported_request(&request) {
                                let _ = client
                                    .reject_server_request(
                                        request.id().clone(),
                                        JSONRPCErrorError {
                                            code: -32601,
                                            message: "desktop app-server client does not provide this server-request capability yet".to_string(),
                                            data: None,
                                        },
                                    )
                                    .await;
                                continue;
                            }

                            sessions.add_pending_request(&session_id, request.clone()).await;
                            let envelope = AgentServerRequestEnvelope {
                                session_id: session_id.clone(),
                                run_id: session_id.clone(),
                                request,
                            };
                            let _ = app.emit(AGENT_SERVER_REQUEST, envelope);
                        }
                        AppServerEvent::Disconnected { message } => {
                            emit_disconnected(&app, &session_id, &message);
                            sessions.mark_disconnected(&session_id).await;
                            break;
                        }
                    }
                }
            }
        }
    })
}

fn should_reject_unsupported_request(request: &ServerRequest) -> bool {
    matches!(
        request,
        ServerRequest::DynamicToolCall { .. }
            | ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::AttestationGenerate { .. }
    )
}

fn emit_event(app: &AppHandle, session_id: &str, event: AgentServerEventPayload) {
    let envelope = AgentServerEventEnvelope {
        session_id: session_id.to_string(),
        run_id: session_id.to_string(),
        event,
    };
    let _ = app.emit(AGENT_SERVER_EVENT, envelope);
}

fn emit_disconnected(app: &AppHandle, session_id: &str, message: &str) {
    let disconnected = AgentServerDisconnectedEnvelope {
        session_id: session_id.to_string(),
        run_id: session_id.to_string(),
        message: message.to_string(),
    };
    let _ = app.emit(AGENT_SERVER_DISCONNECTED, disconnected);
    emit_event(
        app,
        session_id,
        AgentServerEventPayload::Disconnected {
            message: message.to_string(),
        },
    );
}
