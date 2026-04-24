//! LLM text generation service.
//!
//! Supports OpenAI and Anthropic for text generation with a unified interface.

use crate::config::SETTINGS;
use crate::http_error::read_failed_response_text;
use crate::models::{AppError, LlmProvider, LlmUsage, TextGenerationResponse};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Trait for LLM text generation services.
#[async_trait]
pub trait LlmService: Send + Sync {
    /// Generate text from a prompt.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The input prompt
    /// * `model` - Model to use
    /// * `max_tokens` - Maximum tokens to generate
    /// * `temperature` - Sampling temperature (0.0-2.0)
    ///
    /// # Errors
    ///
    /// Returns an error if generation fails.
    async fn generate_text(
        &self,
        prompt: &str,
        model: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<TextGenerationResponse, AppError>;
}

/// OpenAI text generation service.
pub struct OpenAiService {
    client: Client,
    api_key: String,
}

impl OpenAiService {
    /// Create a new OpenAI service.
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
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, api_key })
    }
}

#[async_trait]
impl LlmService for OpenAiService {
    async fn generate_text(
        &self,
        prompt: &str,
        model: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<TextGenerationResponse, AppError> {
        #[derive(Serialize)]
        struct ChatMessage<'a> {
            role: &'a str,
            content: &'a str,
        }

        #[derive(Serialize)]
        struct OpenAiRequest<'a> {
            model: &'a str,
            messages: Vec<ChatMessage<'a>>,
            max_tokens: u32,
            temperature: f32,
        }

        #[derive(Deserialize)]
        struct OpenAiResponse {
            choices: Vec<Choice>,
            usage: Usage,
        }

        #[derive(Deserialize)]
        struct Choice {
            message: MessageContent,
        }

        #[derive(Deserialize)]
        struct MessageContent {
            content: String,
        }

        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
        }

        let request = OpenAiRequest {
            model,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
            max_tokens,
            temperature,
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("OpenAI request failed: {e}")))?;

        if !response.status().is_success() {
            let (status, body) = read_failed_response_text(response).await?;
            return Err(AppError::ExternalService(format!(
                "OpenAI API error (status {status}): {body}"
            )));
        }

        let openai_response: OpenAiResponse = response.json().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to parse OpenAI response: {e}"))
        })?;

        let text = openai_response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| AppError::ExternalService("No response from OpenAI".to_string()))?;

        Ok(TextGenerationResponse {
            text,
            model: model.to_string(),
            usage: LlmUsage::completion(
                openai_response.usage.prompt_tokens,
                openai_response.usage.completion_tokens,
            ),
        })
    }
}

/// Anthropic (Claude) text generation service.
pub struct AnthropicService {
    client: Client,
    api_key: String,
}

impl AnthropicService {
    /// Create a new Anthropic service.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(api_key: String) -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, api_key })
    }
}

#[async_trait]
impl LlmService for AnthropicService {
    async fn generate_text(
        &self,
        prompt: &str,
        model: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<TextGenerationResponse, AppError> {
        #[derive(Serialize)]
        struct AnthropicMessage<'a> {
            role: &'a str,
            content: &'a str,
        }

        #[derive(Serialize)]
        struct AnthropicRequest<'a> {
            model: &'a str,
            messages: Vec<AnthropicMessage<'a>>,
            max_tokens: u32,
            temperature: f32,
        }

        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<ContentBlock>,
            usage: AnthropicUsage,
        }

        #[derive(Deserialize)]
        struct ContentBlock {
            text: String,
        }

        #[derive(Deserialize)]
        struct AnthropicUsage {
            input_tokens: u32,
            output_tokens: u32,
        }

        let request = AnthropicRequest {
            model,
            messages: vec![AnthropicMessage {
                role: "user",
                content: prompt,
            }],
            max_tokens,
            temperature,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Anthropic request failed: {e}")))?;

        if !response.status().is_success() {
            let (status, body) = read_failed_response_text(response).await?;
            return Err(AppError::ExternalService(format!(
                "Anthropic API error (status {status}): {body}"
            )));
        }

        let anthropic_response: AnthropicResponse = response.json().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to parse Anthropic response: {e}"))
        })?;

        let text = anthropic_response
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .ok_or_else(|| AppError::ExternalService("No response from Anthropic".to_string()))?;

        Ok(TextGenerationResponse {
            text,
            model: model.to_string(),
            usage: LlmUsage::completion(
                anthropic_response.usage.input_tokens,
                anthropic_response.usage.output_tokens,
            ),
        })
    }
}

/// Factory for creating LLM service instances.
pub struct LlmServiceFactory;

impl LlmServiceFactory {
    /// Get an LLM service for the specified provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - The LLM provider to use
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not configured.
    pub fn get_service(provider: LlmProvider) -> Result<Box<dyn LlmService>, AppError> {
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
                Ok(Box::new(OpenAiService::new(api_key)?))
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
                Ok(Box::new(AnthropicService::new(api_key)?))
            }
        }
    }
}
