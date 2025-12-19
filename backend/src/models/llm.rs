//! LLM (Large Language Model) related models.
//!
//! Types for text generation and embedding requests/responses.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Supported LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    #[default]
    OpenAI,
    Anthropic,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenAI => write!(f, "openai"),
            Self::Anthropic => write!(f, "anthropic"),
        }
    }
}

/// Token usage information for LLM API calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    /// Number of tokens in the prompt
    pub prompt_tokens: u32,

    /// Number of tokens in the completion (None for embeddings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,

    /// Total tokens used
    pub total_tokens: u32,
}

impl LlmUsage {
    /// Create usage stats for a completion request.
    #[must_use]
    pub const fn completion(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens: Some(completion_tokens),
            total_tokens: prompt_tokens + completion_tokens,
        }
    }

    /// Create usage stats for an embedding request.
    #[must_use]
    pub const fn embedding(prompt_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens: None,
            total_tokens: prompt_tokens,
        }
    }
}

/// Request for text generation.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TextGenerationRequest {
    /// The prompt to generate text from
    #[validate(length(min = 1, message = "Prompt cannot be empty"))]
    pub prompt: String,

    /// Model to use for generation
    #[serde(default = "default_text_model")]
    pub model: String,

    /// Maximum tokens to generate
    #[serde(default = "default_max_tokens")]
    #[validate(range(min = 1, max = 4000, message = "max_tokens must be between 1 and 4000"))]
    pub max_tokens: u32,

    /// Temperature for generation (0.0-2.0)
    #[serde(default = "default_temperature")]
    #[validate(range(
        min = 0.0,
        max = 2.0,
        message = "temperature must be between 0.0 and 2.0"
    ))]
    pub temperature: f32,

    /// LLM provider to use
    #[serde(default)]
    pub provider: LlmProvider,
}

fn default_text_model() -> String {
    "gpt-3.5-turbo".to_string()
}

const fn default_max_tokens() -> u32 {
    500
}

const fn default_temperature() -> f32 {
    0.7
}

/// Response from text generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextGenerationResponse {
    /// Generated text
    pub text: String,

    /// Model used for generation
    pub model: String,

    /// Token usage statistics
    pub usage: LlmUsage,
}

/// Request for creating an embedding.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct EmbeddingRequest {
    /// Text to create embedding for
    #[validate(length(min = 1, message = "Text cannot be empty"))]
    pub text: String,

    /// Model to use for embedding
    #[serde(default = "default_embedding_model")]
    pub model: String,

    /// Provider to use
    #[serde(default)]
    pub provider: LlmProvider,
}

fn default_embedding_model() -> String {
    "text-embedding-ada-002".to_string()
}

/// Response from embedding creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// The embedding vector
    pub embedding: Vec<f32>,

    /// Model used for embedding
    pub model: String,

    /// Token usage statistics
    pub usage: LlmUsage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_usage_completion() {
        let usage = LlmUsage::completion(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, Some(50));
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_llm_usage_embedding() {
        let usage = LlmUsage::embedding(100);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, None);
        assert_eq!(usage.total_tokens, 100);
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(LlmProvider::OpenAI.to_string(), "openai");
        assert_eq!(LlmProvider::Anthropic.to_string(), "anthropic");
    }
}
