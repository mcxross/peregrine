use std::collections::HashMap;
use std::sync::Arc;

use seal_sdk_rs::base_client::BaseSealClient;
use seal_sdk_rs::base_client::KeyServerConfig;
use seal_sdk_rs::cache::NoCache;
use seal_sdk_rs::error::SealClientError;
use seal_sdk_rs::generic_types::ObjectID;

use crate::error::MemWalError;
use crate::sui::ApprovalPtb;
use crate::sui::CurrentSuiClientAdapter;
use crate::types::SealServerConfig;

pub(crate) struct SealService {
    client: BaseSealClient<
        NoCache<
            seal_sdk_rs::cache_key::KeyServerInfoCacheKey,
            seal_sdk_rs::base_client::KeyServerInfo,
        >,
        NoCache<seal_sdk_rs::cache_key::DerivedKeyCacheKey, seal_sdk_rs::base_client::DerivedKeys>,
        SealClientError,
        CurrentSuiClientAdapter,
        SealClientError,
        SealReqwestClient,
    >,
    key_servers: Vec<SealServerConfig>,
    threshold: u8,
}

impl SealService {
    pub(crate) fn new(
        rpc_client: sui_rpc::Client,
        http_client: reqwest::Client,
        key_servers: Vec<SealServerConfig>,
        threshold: u8,
    ) -> Self {
        let client = BaseSealClient::new_custom(
            NoCache::default(),
            NoCache::default(),
            CurrentSuiClientAdapter::new(rpc_client),
            SealReqwestClient::new(http_client, key_servers.clone()),
        );
        Self {
            client,
            key_servers,
            threshold,
        }
    }

    pub(crate) async fn encrypt(
        &self,
        package_id: sui_sdk_types::Address,
        id: Vec<u8>,
        plaintext: Vec<u8>,
    ) -> Result<Vec<u8>, MemWalError> {
        let (encrypted, _recovery_key) = self
            .client
            .encrypt_bytes(
                ObjectID(package_id.into_inner()),
                id,
                self.threshold,
                self.key_server_configs(),
                plaintext,
            )
            .await
            .map_err(|error| MemWalError::seal(error.to_string()))?;
        bcs::to_bytes(&encrypted).map_err(Into::into)
    }

    pub(crate) async fn decrypt_many(
        &self,
        encrypted_objects: &[Vec<u8>],
        approval_ptb: ApprovalPtb,
        session_key: &seal_sdk_rs::session_key::SessionKey,
    ) -> Result<Vec<Vec<u8>>, MemWalError> {
        let refs = encrypted_objects
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let aggregator_urls = self
            .key_servers
            .iter()
            .filter_map(|config| {
                config
                    .aggregator_url
                    .clone()
                    .map(|url| (ObjectID(config.object_id.into_inner()), url))
            })
            .collect::<HashMap<_, _>>();
        self.client
            .decrypt_multiple_objects_bytes(&refs, approval_ptb, session_key, aggregator_urls)
            .await
            .map_err(|error| MemWalError::seal(error.to_string()))
    }

    fn key_server_configs(&self) -> Vec<KeyServerConfig> {
        self.key_servers
            .iter()
            .map(|config| {
                KeyServerConfig::new(
                    ObjectID(config.object_id.into_inner()),
                    config.aggregator_url.clone(),
                )
            })
            .collect()
    }
}

#[derive(Clone)]
pub(crate) struct SealReqwestClient {
    client: reqwest::Client,
    headers_by_url: Arc<HashMap<String, (String, String)>>,
}

impl SealReqwestClient {
    fn new(http_client: reqwest::Client, key_servers: Vec<SealServerConfig>) -> Self {
        let headers_by_url = key_servers
            .into_iter()
            .filter_map(|config| {
                match (config.aggregator_url, config.api_key_name, config.api_key) {
                    (Some(url), Some(name), Some(value)) => Some((url, (name, value))),
                    _ => None,
                }
            })
            .collect();
        Self {
            client: http_client,
            headers_by_url: Arc::new(headers_by_url),
        }
    }
}

#[async_trait::async_trait]
impl seal_sdk_rs::http_client::HttpClient for SealReqwestClient {
    type PostError = SealClientError;

    async fn post<S: ToString + Send + Sync>(
        &self,
        url: &str,
        headers: HashMap<String, String>,
        body: S,
    ) -> Result<seal_sdk_rs::http_client::PostResponse, Self::PostError> {
        let mut request = self.client.post(url);
        for (key, value) in headers {
            request = request.header(&key, &value);
        }
        if let Some((name, value)) = self.headers_by_url.get(url) {
            request = request.header(name, value);
        }

        let response = request
            .body(body.to_string())
            .send()
            .await
            .map_err(|error| SealClientError::CannotUnwrapTypedError {
                error_message: error.to_string(),
            })?;
        let status = response.status().as_u16();
        let text =
            response
                .text()
                .await
                .map_err(|error| SealClientError::CannotUnwrapTypedError {
                    error_message: error.to_string(),
                })?;
        Ok(seal_sdk_rs::http_client::PostResponse { status, text })
    }
}
