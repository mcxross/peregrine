use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::MemWalError;

pub trait EmbeddingProvider: Send + Sync {
    fn embed<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<f32>, MemWalError>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct OpenAiEmbeddingProvider {
    client: reqwest::Client,
    api_base: String,
    api_key: String,
    model: String,
}

impl OpenAiEmbeddingProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_base: "https://api.openai.com/v1".to_owned(),
            api_key: api_key.into(),
            model: "text-embedding-3-small".to_owned(),
        }
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

impl EmbeddingProvider for OpenAiEmbeddingProvider {
    fn embed<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<f32>, MemWalError>> + Send + 'a>> {
        Box::pin(async move {
            #[derive(serde::Serialize)]
            struct EmbedRequest<'a> {
                model: &'a str,
                input: &'a str,
            }

            #[derive(serde::Deserialize)]
            struct EmbedResponse {
                data: Vec<EmbedDatum>,
            }

            #[derive(serde::Deserialize)]
            struct EmbedDatum {
                embedding: Vec<f32>,
            }

            let response = self
                .client
                .post(format!(
                    "{}/embeddings",
                    self.api_base.trim_end_matches('/')
                ))
                .bearer_auth(&self.api_key)
                .json(&EmbedRequest {
                    model: &self.model,
                    input: text,
                })
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(MemWalError::embedding(
                    response.text().await.unwrap_or_default(),
                ));
            }

            let payload = response.json::<EmbedResponse>().await?;
            payload
                .data
                .into_iter()
                .next()
                .map(|item| item.embedding)
                .ok_or_else(|| MemWalError::embedding("embedding API returned no embedding"))
        })
    }
}

pub(crate) type SharedEmbeddingProvider = Arc<dyn EmbeddingProvider>;
