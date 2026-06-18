use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use peregrine_app_server_protocol::{
    AuditFindingUpdatedNotification, AuditStageUpdatedNotification, ServerNotification,
};
use peregrine_audit_store::{AuditStore, AuditStoreEvent};
use serde_json::Value as JsonValue;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::outgoing_message::OutgoingMessageSender;

const EVENT_RECV_TIMEOUT: Duration = Duration::from_millis(250);

pub(crate) struct AuditEventForwarder {
    shutdown_token: CancellationToken,
    handle: JoinHandle<()>,
}

impl AuditEventForwarder {
    pub(crate) fn start(
        peregrine_home: &Path,
        outgoing: Arc<OutgoingMessageSender>,
    ) -> Option<Self> {
        let store = match AuditStore::open(peregrine_home) {
            Ok(store) => store,
            Err(err) => {
                warn!("audit event forwarder disabled: failed to open audit store: {err}");
                return None;
            }
        };
        let receiver = match store.subscribe_events() {
            Ok(receiver) => receiver,
            Err(err) => {
                warn!("audit event forwarder disabled: failed to subscribe to audit store: {err}");
                return None;
            }
        };
        let shutdown_token = CancellationToken::new();
        let child_token = shutdown_token.child_token();
        let handle = tokio::task::spawn_blocking(move || {
            while !child_token.is_cancelled() {
                match receiver.recv_timeout(EVENT_RECV_TIMEOUT) {
                    Ok(event) => forward_event(&outgoing, event),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Some(Self {
            shutdown_token,
            handle,
        })
    }

    pub(crate) async fn shutdown(self) {
        self.shutdown_token.cancel();
        if let Err(err) = self.handle.await {
            warn!("audit event forwarder shutdown failed: {err}");
        }
    }
}

fn forward_event(outgoing: &OutgoingMessageSender, event: AuditStoreEvent) {
    match event {
        AuditStoreEvent::StageUpdated {
            audit_id,
            stage,
            status,
            run,
        } => {
            outgoing.try_send_server_notification(ServerNotification::AuditStageUpdated(
                AuditStageUpdatedNotification {
                    audit_id,
                    stage: json_value(stage),
                    status: json_value(status),
                    run: json_value(run),
                },
            ));
        }
        AuditStoreEvent::FindingUpdated {
            audit_id,
            finding,
            report_ref,
        } => {
            let finding_id = finding.id.clone();
            outgoing.try_send_server_notification(ServerNotification::AuditFindingUpdated(
                AuditFindingUpdatedNotification {
                    audit_id,
                    finding_id,
                    finding: json_value(finding),
                    report_ref: Some(report_ref),
                },
            ));
        }
    }
}

fn json_value(value: impl serde::Serialize) -> JsonValue {
    serde_json::to_value(value).unwrap_or_else(|error| {
        serde_json::json!({
            "serializationError": error.to_string(),
        })
    })
}
