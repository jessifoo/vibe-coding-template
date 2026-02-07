//! Text-generation service (`OpenAI` & Anthropic).

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
// OpenAI
// ---------------------------------------------------------------------------

pub struct OpenAiService {
    client: Client,
    api_key: String,
}

impl OpenAiService {
    fn new(api_key: String) -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("HTTP client error: {e}")))?;
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
            choices: Vec<Choice>,
            usage: Usage,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Content,
        }
        #[derive(Deserialize)]
        struct Content {
            content: String,
        }
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
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
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("OpenAI request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "OpenAI API error ({status}): {text}"
            )));
        }

        let data: Resp = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("OpenAI parse error: {e}")))?;

        let text = data
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| AppError::ExternalService("No response from OpenAI".into()))?;

        Ok(TextGenerationResponse {
            text,
            model: model.to_string(),
            usage: LlmUsage::completion(data.usage.prompt_tokens, data.usage.completion_tokens),
        })
    }
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

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
    /// Build a boxed service for `provider`, or return a config error.
    pub fn create(provider: LlmProvider) -> Result<Box<dyn LlmService>, AppError> {
        match provider {
            LlmProvider::OpenAI => {
                let key = require_api_key(SETTINGS.llm.openai_api_key.as_ref(), "OPENAI_API_KEY")?;
                Ok(Box::new(OpenAiService::new(key)?))
            }
            LlmProvider::Anthropic => {
                let key =
                    require_api_key(SETTINGS.llm.anthropic_api_key.as_ref(), "ANTHROPIC_API_KEY")?;
                Ok(Box::new(AnthropicService::new(key)?))
            }
        }
    }
}

/// Extract a non-empty API key or return a configuration error.
fn require_api_key(slot: Option<&String>, var_name: &str) -> Result<String, AppError> {
    slot.filter(|k| !k.is_empty())
        .cloned()
        .ok_or_else(|| AppError::Configuration(format!("{var_name} not configured")))
}
