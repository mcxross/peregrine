use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use chrono::TimeZone;
use chrono::Utc;
use tokio::sync::Mutex;
use tokio::sync::Notify;

use crate::error::MemWalError;
use crate::sui::Ed25519Signer;
use crate::sui::MemWalSigner;
use crate::types::RelayerConfig;

const SESSION_TTL_MINUTES: u16 = 5;
const SESSION_REFRESH_EARLY: Duration = Duration::from_secs(30);

pub(crate) trait SealHeaderProvider: Send + Sync {
    fn seal_header_value<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<String, MemWalError>> + Send + 'a>>;
}

#[derive(Clone)]
pub(crate) struct SealSessionManager {
    rpc_url: String,
    package_id: sui_sdk_types::Address,
    signer: Arc<dyn MemWalSigner>,
    state: Arc<Mutex<SessionState>>,
    notify: Arc<Notify>,
}

#[derive(Debug, Default)]
struct SessionState {
    cached: Option<CachedSession>,
    building: bool,
}

#[derive(Debug, Clone)]
struct CachedSession {
    header_value: String,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedSessionKey {
    address: String,
    package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mvr_name: Option<String>,
    creation_time_ms: u64,
    ttl_min: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    personal_message_signature: Option<String>,
    session_key: String,
}

impl SealSessionManager {
    pub(crate) fn for_delegate_key(
        delegate_key: crate::auth::DelegateKey,
        config: &RelayerConfig,
    ) -> Self {
        let signer = Arc::new(Ed25519Signer::from_delegate_key(delegate_key));
        Self::new(config.package_id, config.sui_rpc_url.clone(), signer)
    }

    pub(crate) fn new(
        package_id: sui_sdk_types::Address,
        rpc_url: String,
        signer: Arc<dyn MemWalSigner>,
    ) -> Self {
        Self {
            rpc_url,
            package_id,
            signer,
            state: Arc::new(Mutex::new(SessionState::default())),
            notify: Arc::new(Notify::new()),
        }
    }

    async fn build_session(&self) -> Result<String, MemWalError> {
        let rpc = sui_rpc::Client::new(self.rpc_url.as_str())?;
        let request = sui_rpc::proto::sui::rpc::v2::GetObjectRequest::new(&self.package_id.into());
        let response = rpc
            .clone()
            .ledger_client()
            .get_object(request)
            .await?
            .into_inner();
        let package = response
            .object
            .ok_or_else(|| MemWalError::sui_rpc(tonic::Status::not_found("package not found")))?;
        if package.version != Some(1) {
            return Err(MemWalError::compatibility(format!(
                "unexpected MemWal package object version {:?} for {}",
                package.version, self.package_id
            )));
        }

        let creation_time_ms = Utc::now().timestamp_millis() as u64;
        let ephemeral = Ed25519Signer::generate();
        let public_key_bytes = ephemeral.public_key_bytes();
        let public_key_base64 = base64::engine::general_purpose::STANDARD.encode(public_key_bytes);
        let timestamp = Utc
            .timestamp_millis_opt(creation_time_ms as i64)
            .single()
            .ok_or_else(|| MemWalError::crypto("invalid session creation time"))?;
        let message = format!(
            "Accessing keys of package {} for {} mins from {}, session key {}",
            self.package_id,
            SESSION_TTL_MINUTES,
            timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            public_key_base64
        );
        let signature = self
            .signer
            .sign_personal_message(&sui_sdk_types::PersonalMessage(message.as_bytes().into()))?;

        let exported = ExportedSessionKey {
            address: self.signer.address()?.to_string(),
            package_id: self.package_id.to_string(),
            mvr_name: None,
            creation_time_ms,
            ttl_min: SESSION_TTL_MINUTES,
            personal_message_signature: Some(signature.to_base64()),
            session_key: ephemeral.to_suiprivkey()?,
        };

        crate::utils::encode_base64_json(&exported).map_err(Into::into)
    }
}

impl SealHeaderProvider for SealSessionManager {
    fn seal_header_value<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<String, MemWalError>> + Send + 'a>> {
        Box::pin(async move {
            loop {
                let wait = {
                    let mut state = self.state.lock().await;
                    if let Some(cached) = &state.cached {
                        if cached.expires_at
                            - chrono::Duration::from_std(SESSION_REFRESH_EARLY).expect("duration")
                            > Utc::now()
                        {
                            return Ok(cached.header_value.clone());
                        }
                    }

                    if state.building {
                        Some(self.notify.notified())
                    } else {
                        state.building = true;
                        None
                    }
                };

                if let Some(wait) = wait {
                    wait.await;
                    continue;
                }

                let built = self.build_session().await;
                let mut state = self.state.lock().await;
                state.building = false;
                if let Ok(header_value) = &built {
                    state.cached = Some(CachedSession {
                        header_value: header_value.clone(),
                        expires_at: Utc::now()
                            + chrono::Duration::minutes(i64::from(SESSION_TTL_MINUTES)),
                    });
                }
                self.notify.notify_waiters();
                return built;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::ExportedSessionKey;

    #[test]
    fn exported_session_json_uses_expected_keys() {
        let encoded = crate::utils::encode_base64_json(&ExportedSessionKey {
            address: "0x1".to_owned(),
            package_id: "0x2".to_owned(),
            mvr_name: None,
            creation_time_ms: 1,
            ttl_min: 5,
            personal_message_signature: Some("sig".to_owned()),
            session_key: "suiprivkey".to_owned(),
        })
        .expect("encode");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("decode");
        let json = String::from_utf8(decoded).expect("utf8");
        assert!(json.contains("\"packageId\":\"0x2\""));
        assert!(json.contains("\"personalMessageSignature\":\"sig\""));
    }
}
