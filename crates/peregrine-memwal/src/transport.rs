use std::sync::Arc;

use reqwest::Method;
use reqwest::StatusCode;
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use url::Url;

use crate::auth::RequestSigner;
use crate::auth::SealHeaderProvider;
use crate::compatibility::assert_compatible_relayer;
use crate::compatibility::compatibility_error_from_status;
use crate::error::MemWalError;
use crate::types::HealthResult;
use crate::types::RelayerConfig;
use crate::types::RelayerVersionMetadata;
use crate::utils::normalize_server_url;
use crate::utils::sanitize_server_error;

#[derive(Clone)]
pub(crate) struct RelayerTransport {
    base_url: Url,
    http: reqwest::Client,
    compatibility: Arc<Mutex<Option<RelayerVersionMetadata>>>,
    public_config: Arc<Mutex<Option<RelayerConfig>>>,
}

impl RelayerTransport {
    pub(crate) fn new(server_url: &str, http: reqwest::Client) -> Result<Self, MemWalError> {
        Ok(Self {
            base_url: normalize_server_url(server_url)?,
            http,
            compatibility: Arc::new(Mutex::new(None)),
            public_config: Arc::new(Mutex::new(None)),
        })
    }

    pub(crate) async fn health(&self) -> Result<HealthResult, MemWalError> {
        self.public_json("health").await
    }

    pub(crate) async fn relayer_config(&self) -> Result<RelayerConfig, MemWalError> {
        if let Some(config) = self.public_config.lock().await.clone() {
            return Ok(config);
        }

        let config: RelayerConfig = self.public_json("config").await?;
        *self.public_config.lock().await = Some(config.clone());
        Ok(config)
    }

    pub(crate) async fn ensure_compatible(&self) -> Result<RelayerVersionMetadata, MemWalError> {
        if let Some(metadata) = self.compatibility.lock().await.clone() {
            return Ok(metadata);
        }

        let version_response = self.http.get(self.join("version")?).send().await?;
        let metadata = if version_response.status().is_success() {
            version_response.json::<RelayerVersionMetadata>().await?
        } else if matches!(
            version_response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ) {
            let health = self.health().await?;
            RelayerVersionMetadata {
                relayer_version: health.relayer_version.unwrap_or(health.version),
                api_version: health.api_version.ok_or_else(|| {
                    MemWalError::compatibility("health response missing apiVersion")
                })?,
                min_supported_sdk: health.min_supported_sdk.ok_or_else(|| {
                    MemWalError::compatibility("health response missing minSupportedSdk")
                })?,
                feature_flags: health.feature_flags.unwrap_or_default(),
                deprecations: health.deprecations.unwrap_or_default(),
                build: health.build.unwrap_or_default(),
            }
        } else {
            return Err(MemWalError::compatibility(format!(
                "MemWal compatibility check failed: GET /version returned {}",
                version_response.status()
            )));
        };

        assert_compatible_relayer(&metadata, self.base_url.as_str())?;
        *self.compatibility.lock().await = Some(metadata.clone());
        Ok(metadata)
    }

    pub(crate) async fn signed_json<B, R>(
        &self,
        signer: &RequestSigner,
        method: Method,
        path: &str,
        body: Option<&B>,
        accepted_statuses: &[StatusCode],
        seal_header: Option<&dyn SealHeaderProvider>,
    ) -> Result<R, MemWalError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.ensure_compatible().await?;

        let body_bytes = if method == Method::GET {
            Vec::new()
        } else if let Some(body) = body {
            serde_json::to_vec(body)?
        } else {
            Vec::new()
        };

        let mut headers = signer.signed_headers(&method, path, &body_bytes)?;
        headers.insert(
            CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        if let Some(provider) = seal_header {
            headers.insert(
                reqwest::header::HeaderName::from_static("x-seal-session"),
                reqwest::header::HeaderValue::from_str(&provider.seal_header_value().await?)
                    .map_err(|error| MemWalError::config(error.to_string()))?,
            );
        }

        let request = self
            .http
            .request(method.clone(), self.join(path.trim_start_matches('/'))?)
            .headers(headers);
        let request = if method == Method::GET {
            request
        } else {
            request.body(body_bytes)
        };
        let response = request.send().await?;
        self.handle_json_response(response, accepted_statuses).await
    }

    pub(crate) async fn public_json<R>(&self, path: &str) -> Result<R, MemWalError>
    where
        R: DeserializeOwned,
    {
        let response = self
            .http
            .get(self.join(path.trim_start_matches('/'))?)
            .send()
            .await?;
        self.handle_json_response(response, &[StatusCode::OK]).await
    }

    fn join(&self, path: &str) -> Result<Url, MemWalError> {
        self.base_url.join(path).map_err(Into::into)
    }

    async fn handle_json_response<R>(
        &self,
        response: reqwest::Response,
        accepted_statuses: &[StatusCode],
    ) -> Result<R, MemWalError>
    where
        R: DeserializeOwned,
    {
        if !accepted_statuses.contains(&response.status()) {
            let status = response.status().as_u16();
            let raw = response.text().await.unwrap_or_default();
            if let Some(error) = compatibility_error_from_status(status, &raw) {
                return Err(error);
            }

            let (message, server_code) = sanitize_server_error(status, &raw);
            return Err(MemWalError::RelayerStatus {
                status,
                message,
                server_code,
                raw,
            });
        }

        response.json::<R>().await.map_err(Into::into)
    }
}
