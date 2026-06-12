use std::sync::Arc;

use base64::Engine;
use reqwest::Method;
use reqwest::StatusCode;
use seal_sdk_rs::session_key::SessionKey;
use sui_rpc::field::FieldMask;
use sui_rpc::field::FieldMaskUtil;
use sui_rpc::proto::sui::rpc::v2::GetObjectRequest;

use crate::auth::DelegateKey;
use crate::auth::RequestSigner;
use crate::auth::SealSessionManager;
use crate::error::MemWalError;
use crate::manual::embedding::SharedEmbeddingProvider;
use crate::manual::seal::SealService;
use crate::manual::walrus::SharedWalrusStore;
use crate::sui::MemWalSigner;
use crate::sui::SealSignerAdapter;
use crate::sui::build_seal_approve_ptb;
use crate::sui::shared_object_version;
use crate::transport::RelayerTransport;
use crate::types::ManualEncryptedRegisterRequest;
use crate::types::ManualRecallFailure;
use crate::types::ManualRecallFailureStage;
use crate::types::ManualRecallMemory;
use crate::types::ManualRecallOptions;
use crate::types::ManualRecallResultWithFailures;
use crate::types::RecallManualResult;
use crate::types::RecallVectorRequest;
use crate::types::RelayerVersionMetadata;
use crate::types::RememberManualResult;
use crate::types::RestoreResult;
use crate::types::SealServerConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuiNetwork {
    Mainnet,
    Testnet,
}

pub struct MemWalManualConfig {
    pub delegate_key: DelegateKey,
    pub signer: Arc<dyn MemWalSigner>,
    pub package_id: sui_sdk_types::Address,
    pub account_id: sui_sdk_types::Address,
    pub network: SuiNetwork,
    pub sui_rpc_url: Option<String>,
    pub server_url: String,
    pub namespace: String,
    pub http_client: reqwest::Client,
    pub embedding_provider: SharedEmbeddingProvider,
    pub walrus_store: SharedWalrusStore,
    pub seal_servers: Option<Vec<SealServerConfig>>,
    pub seal_threshold: Option<u8>,
}

impl MemWalManualConfig {
    pub fn new(
        delegate_key: DelegateKey,
        signer: Arc<dyn MemWalSigner>,
        package_id: sui_sdk_types::Address,
        account_id: sui_sdk_types::Address,
        network: SuiNetwork,
        embedding_provider: SharedEmbeddingProvider,
        walrus_store: SharedWalrusStore,
    ) -> Self {
        Self {
            delegate_key,
            signer,
            package_id,
            account_id,
            network,
            sui_rpc_url: None,
            server_url: "https://relayer.memwal.ai/".to_owned(),
            namespace: "default".to_owned(),
            http_client: reqwest::Client::new(),
            embedding_provider,
            walrus_store,
            seal_servers: None,
            seal_threshold: None,
        }
    }
}

pub struct MemWalManual {
    transport: RelayerTransport,
    signer: RequestSigner,
    owner_signer: Arc<dyn MemWalSigner>,
    session_manager: Arc<SealSessionManager>,
    namespace: String,
    package_id: sui_sdk_types::Address,
    account_id: sui_sdk_types::Address,
    rpc_client: sui_rpc::Client,
    embedder: SharedEmbeddingProvider,
    walrus: SharedWalrusStore,
    seal: SealService,
}

impl MemWalManual {
    pub fn new(config: MemWalManualConfig) -> Result<Self, MemWalError> {
        let transport = RelayerTransport::new(&config.server_url, config.http_client.clone())?;
        let signer = RequestSigner::new(config.delegate_key.clone(), config.account_id);
        let rpc_url = config.sui_rpc_url.unwrap_or_else(|| match config.network {
            SuiNetwork::Mainnet => sui_rpc::Client::MAINNET_FULLNODE.to_owned(),
            SuiNetwork::Testnet => sui_rpc::Client::TESTNET_FULLNODE.to_owned(),
        });
        let rpc_client = sui_rpc::Client::new(rpc_url.as_str())?;
        let key_servers = config
            .seal_servers
            .unwrap_or_else(|| default_seal_servers(config.network));
        let threshold = config.seal_threshold.unwrap_or_else(|| {
            std::cmp::min(
                2,
                key_servers
                    .iter()
                    .map(|entry| entry.weight)
                    .sum::<u8>()
                    .max(1),
            )
        });
        let session_manager = Arc::new(SealSessionManager::new(
            config.package_id,
            rpc_url,
            config.signer.clone(),
        ));
        let seal = SealService::new(
            rpc_client.clone(),
            config.http_client,
            key_servers,
            threshold,
        );

        Ok(Self {
            transport,
            signer,
            owner_signer: config.signer,
            session_manager,
            namespace: config.namespace,
            package_id: config.package_id,
            account_id: config.account_id,
            rpc_client,
            embedder: config.embedding_provider,
            walrus: config.walrus_store,
            seal,
        })
    }

    pub async fn compatibility(&self) -> Result<RelayerVersionMetadata, MemWalError> {
        self.transport.ensure_compatible().await
    }

    pub async fn remember(
        &self,
        text: &str,
        namespace: Option<&str>,
    ) -> Result<RememberManualResult, MemWalError> {
        let namespace = namespace.unwrap_or(&self.namespace);
        let (vector, encrypted) = tokio::try_join!(
            self.embedder.embed(text),
            self.seal.encrypt(
                self.package_id,
                approval_scope_id(namespace, self.owner_signer.address()?),
                text.as_bytes().to_vec()
            ),
        )?;
        let encrypted_base64 = base64::engine::general_purpose::STANDARD.encode(encrypted);
        let body = ManualEncryptedRegisterRequest {
            encrypted_data: &encrypted_base64,
            vector: &vector,
            namespace,
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

    pub async fn recall(
        &self,
        query: &str,
        options: ManualRecallOptions,
    ) -> Result<ManualRecallResultWithFailures, MemWalError> {
        let namespace = options.namespace.as_deref().unwrap_or(&self.namespace);
        let vector = self.embedder.embed(query).await?;
        let body = RecallVectorRequest {
            vector: &vector,
            limit: options.limit.unwrap_or(10),
            namespace,
            scoring_weights: options.scoring_weights.as_ref(),
        };
        let search: RecallManualResult = self
            .transport
            .signed_json(
                &self.signer,
                Method::POST,
                "/api/recall/manual",
                Some(&body),
                &[StatusCode::OK],
                None,
            )
            .await?;

        if search.results.is_empty() {
            return Ok(ManualRecallResultWithFailures {
                results: Vec::new(),
                total: 0,
                failures: Vec::new(),
            });
        }

        let mut failures = Vec::new();
        let mut downloaded = Vec::new();
        for hit in &search.results {
            match self.walrus.download(&hit.blob_id).await {
                Ok(bytes) => downloaded.push((hit.blob_id.clone(), hit.distance, bytes)),
                Err(error) => failures.push(ManualRecallFailure {
                    blob_id: hit.blob_id.clone(),
                    stage: ManualRecallFailureStage::Download,
                    message: error.to_string(),
                }),
            }
        }

        if downloaded.is_empty() {
            return Ok(ManualRecallResultWithFailures {
                results: Vec::new(),
                total: 0,
                failures,
            });
        }

        let approval_id = approval_scope_id(namespace, self.owner_signer.address()?);
        let account_version = self.account_shared_version().await?;
        let approval_ptb = build_seal_approve_ptb(
            self.package_id,
            self.account_id,
            account_version,
            approval_id,
        )?;
        let mut adapter = SealSignerAdapter::new(self.owner_signer.clone());
        let session_key = SessionKey::new(
            seal_sdk_rs::generic_types::ObjectID(self.package_id.into_inner()),
            5,
            &mut adapter,
        )
        .await
        .map_err(|error| MemWalError::seal(error.to_string()))?;

        let encrypted_objects = downloaded
            .iter()
            .map(|(_, _, bytes)| bytes.clone())
            .collect::<Vec<_>>();
        let decrypted = self
            .seal
            .decrypt_many(&encrypted_objects, approval_ptb, &session_key)
            .await?;

        let mut results = Vec::new();
        for ((blob_id, distance, _), plaintext) in downloaded.into_iter().zip(decrypted) {
            match String::from_utf8(plaintext) {
                Ok(text) => results.push(ManualRecallMemory {
                    blob_id,
                    text,
                    distance,
                }),
                Err(error) => failures.push(ManualRecallFailure {
                    blob_id,
                    stage: ManualRecallFailureStage::Decode,
                    message: error.to_string(),
                }),
            }
        }

        Ok(ManualRecallResultWithFailures {
            total: results.len(),
            results,
            failures,
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

    async fn account_shared_version(&self) -> Result<u64, MemWalError> {
        let request = GetObjectRequest::new(&self.account_id)
            .with_read_mask(FieldMask::from_paths(["owner"]));
        let response = self
            .rpc_client
            .clone()
            .ledger_client()
            .get_object(request)
            .await?
            .into_inner();
        let object = response
            .object
            .ok_or_else(|| MemWalError::config("account object missing"))?;
        let owner = object
            .owner
            .ok_or_else(|| MemWalError::config("account owner missing"))?;
        shared_object_version(&owner)
    }
}

fn default_seal_servers(network: SuiNetwork) -> Vec<SealServerConfig> {
    match network {
        SuiNetwork::Mainnet => vec![
            seal_server("0x145540d931f182fef76467dd8074c9839aea126852d90d18e1556fcbbd1208b6"),
            seal_server("0xe0eb52eba9261b96e895bbb4deca10dcd64fbc626a1133017adcd5131353fd10"),
        ],
        SuiNetwork::Testnet => vec![
            seal_server("0x73d05d62c18d9374e3ea529e8e0ed6161da1a141a94d3f76ae3fe4e99356db75"),
            seal_server("0xf5d14a81a982144ae441cd7d64b09027f116a468bd36e7eca494f750591623c8"),
        ],
    }
}

fn seal_server(object_id: &'static str) -> SealServerConfig {
    SealServerConfig {
        object_id: sui_sdk_types::Address::from_static(object_id),
        weight: 1,
        aggregator_url: None,
        api_key_name: None,
        api_key: None,
    }
}

fn approval_scope_id(namespace: &str, address: sui_sdk_types::Address) -> Vec<u8> {
    let mut bytes = namespace.as_bytes().to_vec();
    bytes.extend_from_slice(address.as_bytes());
    bytes
}
