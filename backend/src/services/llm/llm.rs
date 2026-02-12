//! Text-generation service (`OpenAI` & Anthropic).
//!
//! Uses `async-trait` for the service abstraction so implementations are:
//! - mockable in tests,
//! - swappable at runtime (Open/Closed Principle),
//! - behind a stable trait boundary (Dependency Inversion).
//!
//! `OpenAI` calls go through the [`async_openai`] SDK for full type safety.
//! Anthropic uses `reqwest` directly (no official Rust SDK).

use async_openai::{
    Client as OpenAIClient,
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::SETTINGS;
use crate::models::{AppError, LlmProvider, LlmUsage, TextGenerationResponse};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Unified interface for text-generation backends.
#[async_trait]
pub trait LlmService: Send + Sync {
    /// Generate text from a prompt.
    async fn generate_text(
        &self,
        prompt: &str,
        model: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<TextGenerationResponse, AppError>;
}

// ---------------------------------------------------------------------------
// OpenAI  (via async-openai SDK — typed, retries, streaming-ready)
// ---------------------------------------------------------------------------

/// `OpenAI` text-generation service backed by the [`async_openai`] SDK.
pub struct OpenAiService {
    client: OpenAIClient<OpenAIConfig>,
}

impl OpenAiService {
    /// Create a new service using the given API key.
    fn new(api_key: &str) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        Self {
            client: OpenAIClient::with_config(config),
        }
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
        let message = ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()
            .map_err(|e| AppError::BadRequest(format!("Failed to build message: {e}")))?;

        let request = CreateChatCompletionRequestArgs::default()
            .model(model)
            .max_tokens(max_tokens)
            .temperature(temperature)
            .messages(vec![message.into()])
            .build()
            .map_err(|e| AppError::BadRequest(format!("Failed to build request: {e}")))?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(|e| AppError::ExternalService(format!("OpenAI API error: {e}")))?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| AppError::ExternalService("No response from OpenAI".into()))?;

        let text = choice.message.content.clone().unwrap_or_default();

        let usage = response.usage.map_or_else(
            || LlmUsage::completion(0, 0),
            |u| LlmUsage::completion(u.prompt_tokens, u.completion_tokens),
        );

        Ok(TextGenerationResponse {
            text,
            model: response.model,
            usage,
        })
    }
}

// ---------------------------------------------------------------------------
// Anthropic  (raw reqwest — no official Rust SDK)
// ---------------------------------------------------------------------------

/// Anthropic (Claude) text-generation service.
pub struct AnthropicService {
    client: Client,
    api_key: String,
}

impl AnthropicService {
    fn new(api_key: String) -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("HTTP client error: {e}")))?;
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
        struct Req<'a> {
            model: &'a str,
            messages: Vec<Msg<'a>>,
            max_tokens: u32,
            temperature: f32,
        }
        #[derive(Serialize)]
        struct Msg<'a> {
            role: &'a str,
            content: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            content: Vec<Block>,
            usage: Usage,
        }
        #[derive(Deserialize)]
        struct Block {
            text: String,
        }
        #[derive(Deserialize)]
        struct Usage {
            input_tokens: u32,
            output_tokens: u32,
        }

        let body = Req {
            model,
            messages: vec![Msg {
                role: "user",
                content: prompt,
            }],
            max_tokens,
            temperature,
        };

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Anthropic request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Anthropic API error ({status}): {text}"
            )));
        }

        let data: Resp = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Anthropic parse error: {e}")))?;

        let text = data
            .content
            .into_iter()
            .next()
            .map(|b| b.text)
            .ok_or_else(|| AppError::ExternalService("No response from Anthropic".into()))?;

        Ok(TextGenerationResponse {
            text,
            model: model.to_string(),
            usage: LlmUsage::completion(data.usage.input_tokens, data.usage.output_tokens),
        })
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Creates the appropriate [`LlmService`] for a given provider.
pub struct LlmServiceFactory;

impl LlmServiceFactory {
    /// Build a boxed service for `provider`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Configuration`] if the provider's API key is missing.
    pub fn create(provider: LlmProvider) -> Result<Box<dyn LlmService>, AppError> {
        match provider {
            LlmProvider::OpenAI => {
                let key = require_api_key(SETTINGS.llm.openai_api_key.as_ref(), "OPENAI_API_KEY")?;
                Ok(Box::new(OpenAiService::new(&key)))
            }
            LlmProvider::Anthropic => {
                let key =
                    require_api_key(SETTINGS.llm.anthropic_api_key.as_ref(), "ANTHROPIC_API_KEY")?;
                Ok(Box::new(AnthropicService::new(key)?))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_api_key(slot: Option<&String>, var_name: &str) -> Result<String, AppError> {
    slot.filter(|k| !k.is_empty())
        .cloned()
        .ok_or_else(|| AppError::Configuration(format!("{var_name} not configured")))
}
