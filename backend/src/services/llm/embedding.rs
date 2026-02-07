//! Embedding service (`OpenAI`; Anthropic placeholder).

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::SETTINGS;
use crate::models::{AppError, EmbeddingResponse, LlmProvider, LlmUsage};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Unified interface for embedding backends.
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Create an embedding vector for the given text.
    async fn create_embedding(
        &self,
        text: &str,
        model: &str,
    ) -> Result<EmbeddingResponse, AppError>;
}

// ---------------------------------------------------------------------------
// OpenAI
// ---------------------------------------------------------------------------

pub struct OpenAiEmbeddingService {
    client: Client,
    api_key: String,
}

impl OpenAiEmbeddingService {
    fn new(api_key: String) -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("HTTP client error: {e}")))?;
        Ok(Self { client, api_key })
    }
}

#[async_trait]
impl EmbeddingService for OpenAiEmbeddingService {
    async fn create_embedding(
        &self,
        text: &str,
        model: &str,
    ) -> Result<EmbeddingResponse, AppError> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            input: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            data: Vec<Datum>,
            usage: Usage,
        }
        #[derive(Deserialize)]
        struct Datum {
            embedding: Vec<f32>,
        }
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u32,
        }

        let resp = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&Req { model, input: text })
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("OpenAI embedding request failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "OpenAI embedding API error ({status}): {body}"
            )));
        }

        let data: Resp = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Embedding parse error: {e}")))?;

        let embedding = data
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| AppError::ExternalService("No embedding in response".into()))?;

        Ok(EmbeddingResponse {
            embedding,
            model: model.to_string(),
            usage: LlmUsage::embedding(data.usage.prompt_tokens),
        })
    }
}

// ---------------------------------------------------------------------------
// Anthropic (stub — no dedicated embedding API)
// ---------------------------------------------------------------------------

pub struct AnthropicEmbeddingService;

#[async_trait]
impl EmbeddingService for AnthropicEmbeddingService {
    async fn create_embedding(
        &self,
        _text: &str,
        _model: &str,
    ) -> Result<EmbeddingResponse, AppError> {
        Err(AppError::Configuration(
            "Anthropic does not offer an embeddings API. Use provider 'openai'.".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Creates the appropriate [`EmbeddingService`] for a given provider.
pub struct EmbeddingServiceFactory;

impl EmbeddingServiceFactory {
    /// Build a boxed service for `provider`, or return a config error.
    pub fn create(provider: LlmProvider) -> Result<Box<dyn EmbeddingService>, AppError> {
        match provider {
            LlmProvider::OpenAI => {
                let key = require_api_key(SETTINGS.llm.openai_api_key.as_ref(), "OPENAI_API_KEY")?;
                Ok(Box::new(OpenAiEmbeddingService::new(key)?))
            }
            LlmProvider::Anthropic => {
                require_api_key(SETTINGS.llm.anthropic_api_key.as_ref(), "ANTHROPIC_API_KEY")?;
                Ok(Box::new(AnthropicEmbeddingService))
            }
        }
    }
}

fn require_api_key(slot: Option<&String>, var_name: &str) -> Result<String, AppError> {
    slot.filter(|k| !k.is_empty())
        .cloned()
        .ok_or_else(|| AppError::Configuration(format!("{var_name} not configured")))
}
