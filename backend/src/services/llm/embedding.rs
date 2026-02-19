//! Embedding service (`OpenAI` SDK; Anthropic placeholder).
//!
//! `OpenAI` calls go through the [`async_openai`] SDK for full type safety.
//! Anthropic does not offer a dedicated embeddings API.

use async_openai::{
    Client as OpenAIClient, config::OpenAIConfig, types::CreateEmbeddingRequestArgs,
};
use async_trait::async_trait;

use crate::config::SETTINGS;
use crate::models::{AppError, EmbeddingResponse, LlmProvider, LlmUsage};
use crate::services::common::require_api_key;

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
// OpenAI  (via async-openai SDK)
// ---------------------------------------------------------------------------

/// `OpenAI` embedding service backed by the [`async_openai`] SDK.
pub struct OpenAiEmbeddingService {
    client: OpenAIClient<OpenAIConfig>,
}

impl OpenAiEmbeddingService {
    fn new(api_key: &str) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        Self {
            client: OpenAIClient::with_config(config),
        }
    }
}

#[async_trait]
impl EmbeddingService for OpenAiEmbeddingService {
    async fn create_embedding(
        &self,
        text: &str,
        model: &str,
    ) -> Result<EmbeddingResponse, AppError> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(model)
            .input(text)
            .build()
            .map_err(|e| AppError::BadRequest(format!("Failed to build embedding request: {e}")))?;

        let response = self
            .client
            .embeddings()
            .create(request)
            .await
            .map_err(|e| AppError::ExternalService(format!("OpenAI embedding error: {e}")))?;

        let embedding = response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| AppError::ExternalService("No embedding in response".into()))?;

        let usage = LlmUsage::embedding(response.usage.prompt_tokens);

        Ok(EmbeddingResponse {
            embedding,
            model: model.to_string(),
            usage,
        })
    }
}

// ---------------------------------------------------------------------------
// Anthropic  (stub — no dedicated embedding API)
// ---------------------------------------------------------------------------

/// Anthropic embedding stub — returns a clear error since the API does not exist.
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
    /// Build a boxed service for `provider`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Configuration`] if the provider's API key is missing.
    pub fn create(provider: LlmProvider) -> Result<Box<dyn EmbeddingService>, AppError> {
        match provider {
            LlmProvider::OpenAI => {
                let key = require_api_key(SETTINGS.llm.openai_api_key.as_ref(), "OPENAI_API_KEY")?;
                Ok(Box::new(OpenAiEmbeddingService::new(&key)))
            }
            LlmProvider::Anthropic => {
                require_api_key(SETTINGS.llm.anthropic_api_key.as_ref(), "ANTHROPIC_API_KEY")?;
                Ok(Box::new(AnthropicEmbeddingService))
            }
        }
    }
}
