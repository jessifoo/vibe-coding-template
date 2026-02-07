//! Embedding service for creating vector embeddings.
//!
//! Currently supports OpenAI embeddings. Anthropic placeholder included for future support.

use crate::config::SETTINGS;
use crate::models::{AppError, EmbeddingResponse, LlmProvider, LlmUsage};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Trait for embedding services.
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Create an embedding vector for the given text.
    ///
    /// # Arguments
    ///
    /// * `text` - Text to create embedding for
    /// * `model` - Embedding model to use
    ///
    /// # Errors
    ///
    /// Returns an error if embedding creation fails.
    async fn create_embedding(
        &self,
        text: &str,
        model: &str,
    ) -> Result<EmbeddingResponse, AppError>;
}

/// OpenAI embedding service.
pub struct OpenAiEmbeddingService {
    client: Client,
    api_key: String,
}

impl OpenAiEmbeddingService {
    /// Create a new OpenAI embedding service.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(api_key: String) -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to create HTTP client: {e}")))?;

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
        struct EmbeddingRequest<'a> {
            model: &'a str,
            input: &'a str,
        }

        #[derive(Deserialize)]
        struct OpenAiEmbeddingResponse {
            data: Vec<EmbeddingData>,
            usage: EmbeddingUsage,
        }

        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        #[derive(Deserialize)]
        struct EmbeddingUsage {
            prompt_tokens: u32,
            #[allow(dead_code)]
            total_tokens: u32,
        }

        let request = EmbeddingRequest { model, input: text };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("OpenAI embedding request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "OpenAI embedding API error (status {status}): {error}"
            )));
        }

        let openai_response: OpenAiEmbeddingResponse = response.json().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to parse embedding response: {e}"))
        })?;

        let embedding = openai_response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| AppError::ExternalService("No embedding in response".to_string()))?;

        Ok(EmbeddingResponse {
            embedding,
            model: model.to_string(),
            usage: LlmUsage::embedding(openai_response.usage.prompt_tokens),
        })
    }
}

/// Anthropic embedding service (placeholder - Anthropic doesn't have dedicated embedding API yet).
pub struct AnthropicEmbeddingService {
    #[allow(dead_code)]
    api_key: String,
}

impl AnthropicEmbeddingService {
    /// Create a new Anthropic embedding service.
    ///
    /// Note: This is a placeholder as Anthropic doesn't currently offer a dedicated embedding API.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    #[must_use]
    pub const fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl EmbeddingService for AnthropicEmbeddingService {
    async fn create_embedding(
        &self,
        _text: &str,
        _model: &str,
    ) -> Result<EmbeddingResponse, AppError> {
        // Anthropic doesn't currently have a dedicated embeddings API
        // This returns an error directing users to use OpenAI for embeddings
        Err(AppError::Configuration(
            "Anthropic does not currently offer a dedicated embeddings API. \
             Please use OpenAI (provider: 'openai') for embeddings."
                .to_string(),
        ))
    }
}

/// Factory for creating embedding service instances.
pub struct EmbeddingServiceFactory;

impl EmbeddingServiceFactory {
    /// Get an embedding service for the specified provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - The LLM provider to use
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not configured.
    pub fn get_service(provider: LlmProvider) -> Result<Box<dyn EmbeddingService>, AppError> {
        match provider {
            LlmProvider::OpenAI => {
                let api_key = SETTINGS
                    .llm
                    .openai_api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        AppError::Configuration(
                            "OpenAI API key not configured. Please set OPENAI_API_KEY.".to_string(),
                        )
                    })?;
                Ok(Box::new(OpenAiEmbeddingService::new(api_key)?))
            }
            LlmProvider::Anthropic => {
                let api_key = SETTINGS
                    .llm
                    .anthropic_api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        AppError::Configuration(
                            "Anthropic API key not configured. Please set ANTHROPIC_API_KEY."
                                .to_string(),
                        )
                    })?;
                Ok(Box::new(AnthropicEmbeddingService::new(api_key)))
            }
        }
    }

    /// Get the default embedding service (OpenAI).
    ///
    /// # Errors
    ///
    /// Returns an error if OpenAI is not configured.
    pub fn get_default() -> Result<Box<dyn EmbeddingService>, AppError> {
        Self::get_service(LlmProvider::OpenAI)
    }
}
