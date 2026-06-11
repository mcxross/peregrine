use std::sync::Arc;
use std::time::Duration;

use reqwest::Method;
use reqwest::StatusCode;

use crate::auth::DelegateKey;
use crate::auth::RequestSigner;
use crate::auth::SealSessionManager;
use crate::error::MemWalError;
use crate::transport::RelayerTransport;
use crate::types::AnalyzeOptions;
use crate::types::AnalyzeResult;
use crate::types::AnalyzeWaitResult;
use crate::types::EmbedResult;
use crate::types::HealthResult;
use crate::types::RecallManualResult;
use crate::types::RecallParams;
use crate::types::RecallRequest;
use crate::types::RecallResult;
use crate::types::RecallVectorRequest;
use crate::types::RegisterMemoryRequest;
use crate::types::RelayerVersionMetadata;
use crate::types::RememberAcceptedResult;
use crate::types::RememberBulkAcceptedResult;
use crate::types::RememberBulkItem;
use crate::types::RememberBulkItemResult;
use crate::types::RememberBulkRequest;
use crate::types::RememberBulkResult;
use crate::types::RememberBulkStatusRequest;
use crate::types::RememberBulkStatusResult;
use crate::types::RememberJobState;
use crate::types::RememberJobStatus;
use crate::types::RememberRequest;
use crate::types::RememberResult;
use crate::types::RestoreResult;

#[derive(Clone)]
pub struct MemWal {
    transport: RelayerTransport,
    signer: RequestSigner,
    session_manager: Arc<SealSessionManager>,
    namespace: String,
}

#[derive(Debug)]
pub struct MemWalConfig {
    pub delegate_key: DelegateKey,
    pub account_id: sui_sdk_types::Address,
    pub server_url: String,
    pub namespace: String,
    pub http_client: reqwest::Client,
}

impl MemWalConfig {
    pub fn new(delegate_key: DelegateKey, account_id: sui_sdk_types::Address) -> Self {
        Self {
            delegate_key,
            account_id,
            server_url: "https://relayer.memwal.ai/".to_owned(),
            namespace: "default".to_owned(),
            http_client: reqwest::Client::new(),
        }
    }
}

impl MemWal {
    pub async fn new(config: MemWalConfig) -> Result<Self, MemWalError> {
        let transport = RelayerTransport::new(&config.server_url, config.http_client)?;
        let public_config = transport.relayer_config().await?;
        let session_manager = Arc::new(SealSessionManager::for_delegate_key(
            config.delegate_key.clone(),
            &public_config,
        ));

        Ok(Self {
            transport,
            signer: RequestSigner::new(config.delegate_key, config.account_id),
            session_manager,
            namespace: config.namespace,
        })
    }

    pub async fn remember(&self, text: &str) -> Result<RememberAcceptedResult, MemWalError> {
        let body = RememberRequest {
            text,
            namespace: &self.namespace,
        };
        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/remember",
                Some(&body),
                &[StatusCode::OK, StatusCode::ACCEPTED],
                Some(self.session_manager.as_ref()),
            )
            .await
    }

    pub async fn get_remember_status(
        &self,
        job_id: &str,
    ) -> Result<RememberJobStatus, MemWalError> {
        self.transport
            .signed_json::<(), RememberJobStatus>(
                &self.signer,
                Method::GET,
                &format!("/api/remember/{job_id}"),
                None,
                &[StatusCode::OK],
                Some(self.session_manager.as_ref()),
            )
            .await
            .or_else(|error| match error {
                MemWalError::RelayerStatus { status: 404, .. } => Ok(RememberJobStatus {
                    job_id: job_id.to_owned(),
                    status: RememberJobState::NotFound,
                    owner: None,
                    namespace: None,
                    blob_id: None,
                    error: Some(format!("remember job not found: {job_id}")),
                }),
                other => Err(other),
            })
    }

    pub async fn wait_for_remember_job(
        &self,
        job_id: &str,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<RememberResult, MemWalError> {
        let deadline = std::time::Instant::now() + timeout;
        let mut attempt = 0u32;

        loop {
            if std::time::Instant::now() >= deadline {
                return Err(MemWalError::Timeout {
                    context: format!("for remember job `{job_id}`"),
                    timeout,
                });
            }

            tokio::time::sleep(jitter_delay(poll_interval, attempt)).await;
            attempt = attempt.saturating_add(1);

            match self.get_remember_status(job_id).await? {
                RememberJobStatus {
                    status: RememberJobState::Done,
                    blob_id,
                    owner,
                    namespace,
                    ..
                } => {
                    return Ok(RememberResult {
                        id: job_id.to_owned(),
                        job_id: job_id.to_owned(),
                        blob_id: blob_id.unwrap_or_default(),
                        owner: owner.unwrap_or_default(),
                        namespace: namespace.unwrap_or_else(|| self.namespace.clone()),
                    });
                }
                RememberJobStatus {
                    status: RememberJobState::Failed,
                    error,
                    ..
                } => {
                    return Err(MemWalError::JobFailed {
                        job_id: job_id.to_owned(),
                        message: error.unwrap_or_else(|| "remember job failed".to_owned()),
                    });
                }
                RememberJobStatus {
                    status: RememberJobState::NotFound,
                    ..
                } => {
                    return Err(MemWalError::JobNotFound {
                        job_id: job_id.to_owned(),
                    });
                }
                _ => {}
            }
        }
    }

    pub async fn remember_and_wait(
        &self,
        text: &str,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<RememberResult, MemWalError> {
        let accepted = self.remember(text).await?;
        self.wait_for_remember_job(&accepted.job_id, poll_interval, timeout)
            .await
    }

    pub async fn remember_bulk(
        &self,
        items: &[RememberBulkItem],
    ) -> Result<RememberBulkAcceptedResult, MemWalError> {
        let body = RememberBulkRequest { items };
        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/remember/bulk",
                Some(&body),
                &[StatusCode::OK, StatusCode::ACCEPTED],
                Some(self.session_manager.as_ref()),
            )
            .await
    }

    pub async fn get_remember_bulk_status(
        &self,
        job_ids: &[String],
    ) -> Result<RememberBulkStatusResult, MemWalError> {
        let body = RememberBulkStatusRequest { job_ids };
        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/remember/bulk/status",
                Some(&body),
                &[StatusCode::OK],
                Some(self.session_manager.as_ref()),
            )
            .await
    }

    pub async fn wait_for_remember_jobs(
        &self,
        job_ids: &[String],
        namespaces: &[String],
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<RememberBulkResult, MemWalError> {
        let deadline = std::time::Instant::now() + timeout;
        let mut remaining = job_ids
            .iter()
            .enumerate()
            .map(|(index, job_id)| (job_id.clone(), (index, namespaces[index].clone())))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut results = vec![None; job_ids.len()];
        let mut attempt = 0u32;

        while !remaining.is_empty() {
            if std::time::Instant::now() >= deadline {
                for (job_id, (index, namespace)) in &remaining {
                    results[*index] = Some(RememberBulkItemResult {
                        id: job_id.clone(),
                        blob_id: String::new(),
                        status: crate::types::BulkCompletionState::Timeout,
                        namespace: namespace.clone(),
                        error: Some("timed out".to_owned()),
                    });
                }
                break;
            }

            tokio::time::sleep(jitter_delay(poll_interval, attempt)).await;
            attempt = attempt.saturating_add(1);
            let request_ids = remaining.keys().cloned().collect::<Vec<_>>();
            let status = self.get_remember_bulk_status(&request_ids).await?;
            for item in status.results {
                if let Some((index, namespace)) = remaining.get(&item.job_id).cloned() {
                    match item.status {
                        RememberJobState::Done => {
                            results[index] = Some(RememberBulkItemResult {
                                id: item.job_id.clone(),
                                blob_id: item.blob_id.unwrap_or_default(),
                                status: crate::types::BulkCompletionState::Done,
                                namespace,
                                error: None,
                            });
                            remaining.remove(&item.job_id);
                        }
                        RememberJobState::Failed | RememberJobState::NotFound => {
                            results[index] = Some(RememberBulkItemResult {
                                id: item.job_id.clone(),
                                blob_id: item.blob_id.unwrap_or_default(),
                                status: crate::types::BulkCompletionState::Failed,
                                namespace,
                                error: item.error,
                            });
                            remaining.remove(&item.job_id);
                        }
                        _ => {}
                    }
                }
            }
        }

        let results = results
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                item.unwrap_or(RememberBulkItemResult {
                    id: job_ids[index].clone(),
                    blob_id: String::new(),
                    status: crate::types::BulkCompletionState::Timeout,
                    namespace: namespaces[index].clone(),
                    error: Some("timed out".to_owned()),
                })
            })
            .collect::<Vec<_>>();
        let succeeded = results
            .iter()
            .filter(|item| item.status == crate::types::BulkCompletionState::Done)
            .count();
        let failed = results.len().saturating_sub(succeeded);
        Ok(RememberBulkResult {
            total: results.len(),
            succeeded,
            failed,
            results,
        })
    }

    pub async fn remember_bulk_and_wait(
        &self,
        items: &[RememberBulkItem],
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<RememberBulkResult, MemWalError> {
        let accepted = self.remember_bulk(items).await?;
        let namespaces = items
            .iter()
            .map(|item| {
                item.namespace
                    .clone()
                    .unwrap_or_else(|| self.namespace.clone())
            })
            .collect::<Vec<_>>();
        self.wait_for_remember_jobs(&accepted.job_ids, &namespaces, poll_interval, timeout)
            .await
    }

    pub async fn recall(&self, params: RecallParams) -> Result<RecallResult, MemWalError> {
        let limit = params.top_k.or(params.limit).unwrap_or(10);
        let namespace = params.namespace.as_deref().unwrap_or(&self.namespace);
        let body = RecallRequest {
            query: &params.query,
            limit,
            namespace,
        };

        let mut result: RecallResult = self
            .transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/recall",
                Some(&body),
                &[StatusCode::OK],
                Some(self.session_manager.as_ref()),
            )
            .await?;

        if let Some(max_distance) = params.max_distance {
            result
                .results
                .retain(|memory| memory.distance < max_distance);
            result.total = result.results.len();
        }

        Ok(result)
    }

    pub async fn remember_manual(
        &self,
        blob_id: &str,
        vector: &[f32],
        namespace: Option<&str>,
    ) -> Result<crate::types::RememberManualResult, MemWalError> {
        let ns = namespace.unwrap_or(&self.namespace);
        let body = RegisterMemoryRequest {
            blob_id,
            vector,
            namespace: ns,
        };
        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/remember/manual",
                Some(&body),
                &[StatusCode::OK],
                None,
            )
            .await
    }

    pub async fn recall_manual(
        &self,
        vector: &[f32],
        limit: usize,
        namespace: Option<&str>,
        scoring_weights: Option<&crate::types::ScoringWeights>,
    ) -> Result<RecallManualResult, MemWalError> {
        let ns = namespace.unwrap_or(&self.namespace);
        let body = RecallVectorRequest {
            vector,
            limit,
            namespace: ns,
            scoring_weights,
        };
        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/recall/manual",
                Some(&body),
                &[StatusCode::OK],
                None,
            )
            .await
    }

    pub async fn embed(&self, text: &str) -> Result<EmbedResult, MemWalError> {
        #[derive(serde::Serialize)]
        struct EmbedRequest<'a> {
            text: &'a str,
        }

        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/embed",
                Some(&EmbedRequest { text }),
                &[StatusCode::OK],
                Some(self.session_manager.as_ref()),
            )
            .await
    }

    pub async fn analyze(
        &self,
        text: &str,
        options: AnalyzeOptions,
    ) -> Result<AnalyzeResult, MemWalError> {
        #[derive(serde::Serialize)]
        struct AnalyzeRequest<'a> {
            text: &'a str,
            namespace: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            occurred_at: Option<String>,
        }

        let namespace = options.namespace.as_deref().unwrap_or(&self.namespace);
        let occurred_at = options
            .occurred_at
            .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/analyze",
                Some(&AnalyzeRequest {
                    text,
                    namespace,
                    occurred_at,
                }),
                &[StatusCode::OK, StatusCode::ACCEPTED],
                Some(self.session_manager.as_ref()),
            )
            .await
    }

    pub async fn analyze_and_wait(
        &self,
        text: &str,
        options: AnalyzeOptions,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<AnalyzeWaitResult, MemWalError> {
        let namespace = options
            .namespace
            .clone()
            .unwrap_or_else(|| self.namespace.clone());
        let accepted = self.analyze(text, options).await?;
        let namespaces = vec![namespace; accepted.job_ids.len()];
        let results = self
            .wait_for_remember_jobs(&accepted.job_ids, &namespaces, poll_interval, timeout)
            .await?;
        Ok(AnalyzeWaitResult {
            results,
            facts: accepted.facts,
            owner: accepted.owner,
        })
    }

    pub async fn restore(
        &self,
        namespace: &str,
        limit: usize,
    ) -> Result<RestoreResult, MemWalError> {
        #[derive(serde::Serialize)]
        struct RestoreRequest<'a> {
            namespace: &'a str,
            limit: usize,
        }

        self.transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/restore",
                Some(&RestoreRequest { namespace, limit }),
                &[StatusCode::OK],
                Some(self.session_manager.as_ref()),
            )
            .await
    }

    pub async fn health(&self) -> Result<HealthResult, MemWalError> {
        self.transport.health().await
    }

    pub async fn compatibility(&self) -> Result<RelayerVersionMetadata, MemWalError> {
        self.transport.ensure_compatible().await
    }

    pub fn delegate_public_key_hex(&self) -> String {
        self.signer.delegate_key().public_key_hex()
    }
}

fn jitter_delay(base: Duration, attempt: u32) -> Duration {
    let base_ms = base.as_millis().max(100) as f64;
    let capped = (base_ms * 1.5_f64.powi(attempt.min(6) as i32)).min(10_000.0);
    let jitter = 0.75 + rand::random::<f64>() * 0.5;
    Duration::from_millis((capped * jitter) as u64)
}
