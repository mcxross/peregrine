use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::MemWalError;

pub trait WalrusBlobStore: Send + Sync {
    fn upload<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<String, MemWalError>> + Send + 'a>>;
    fn download<'a>(
        &'a self,
        blob_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, MemWalError>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct WalrusHttpStore {
    client: reqwest::Client,
    publisher_url: String,
    aggregator_url: String,
    epochs: u64,
}

impl WalrusHttpStore {
    pub fn mainnet() -> Self {
        Self {
            client: reqwest::Client::new(),
            publisher_url: "https://publisher.walrus-mainnet.walrus.space".to_owned(),
            aggregator_url: "https://aggregator.walrus-mainnet.walrus.space".to_owned(),
            epochs: 50,
        }
    }

    pub fn testnet() -> Self {
        Self {
            client: reqwest::Client::new(),
            publisher_url: "https://publisher.walrus-testnet.walrus.space".to_owned(),
            aggregator_url: "https://aggregator.walrus-testnet.walrus.space".to_owned(),
            epochs: 50,
        }
    }

    pub fn with_urls(
        mut self,
        publisher_url: impl Into<String>,
        aggregator_url: impl Into<String>,
    ) -> Self {
        self.publisher_url = publisher_url.into();
        self.aggregator_url = aggregator_url.into();
        self
    }

    pub fn with_epochs(mut self, epochs: u64) -> Self {
        self.epochs = epochs;
        self
    }
}

impl WalrusBlobStore for WalrusHttpStore {
    fn upload<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<String, MemWalError>> + Send + 'a>> {
        Box::pin(async move {
            let response = self
                .client
                .put(format!(
                    "{}/v1/blobs?epochs={}&deletable=true",
                    self.publisher_url, self.epochs
                ))
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes.to_vec())
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(MemWalError::walrus(
                    response.text().await.unwrap_or_default(),
                ));
            }

            let payload = response.json::<serde_json::Value>().await?;
            let blob_id = payload
                .pointer("/newlyCreated/blobObject/blobId")
                .and_then(serde_json::Value::as_str)
                .or_else(|| {
                    payload
                        .pointer("/alreadyCertified/blobId")
                        .and_then(serde_json::Value::as_str)
                })
                .ok_or_else(|| {
                    MemWalError::walrus(format!("unexpected Walrus upload response: {payload}"))
                })?;
            Ok(blob_id.to_owned())
        })
    }

    fn download<'a>(
        &'a self,
        blob_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, MemWalError>> + Send + 'a>> {
        Box::pin(async move {
            let response = self
                .client
                .get(format!("{}/v1/blobs/{blob_id}", self.aggregator_url))
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(MemWalError::walrus(
                    response.text().await.unwrap_or_default(),
                ));
            }

            Ok(response.bytes().await?.to_vec())
        })
    }
}

pub(crate) type SharedWalrusStore = Arc<dyn WalrusBlobStore>;
