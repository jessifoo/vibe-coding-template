//! LLM (Large Language Model) related models.
//!
//! Types for text generation and embedding requests/responses.

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Validator to reject empty or whitespace-only strings.
fn validate_non_blank(s: &str) -> Result<(), ValidationError> {
    if s.trim().is_empty() {
        return Err(ValidationError::new("non_blank"));
    }
    Ok(())
}

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
    #[validate(custom(function = "validate_non_blank", message = "Prompt cannot be whitespace only"))]
    pub prompt: String,

    /// Model to use for generation
    #[serde(default = "default_text_model")]
    #[validate(custom(function = "validate_non_blank", message = "Model cannot be whitespace only"))]
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
    "gpt-4o-mini".to_string()
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
    #[validate(custom(function = "validate_non_blank", message = "Text cannot be whitespace only"))]
    pub text: String,

    /// Model to use for embedding
    #[serde(default = "default_embedding_model")]
    #[validate(custom(function = "validate_non_blank", message = "Model cannot be whitespace only"))]
    pub model: String,

    /// Provider to use
    #[serde(default)]
    pub provider: LlmProvider,
}

fn default_embedding_model() -> String {
    crate::models::defaults::default_embedding_model()
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

    #[test]
    fn test_provider_serialization() {
        let provider = LlmProvider::OpenAI;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"openai\"");

        let provider = LlmProvider::Anthropic;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"anthropic\"");
    }

    #[test]
    fn test_provider_deserialization() {
        let provider: LlmProvider = serde_json::from_str("\"openai\"").unwrap();
        assert_eq!(provider, LlmProvider::OpenAI);

        let provider: LlmProvider = serde_json::from_str("\"anthropic\"").unwrap();
        assert_eq!(provider, LlmProvider::Anthropic);
    }

    #[test]
    fn test_llm_usage_serialization() {
        let usage = LlmUsage::completion(100, 50);
        let json = serde_json::to_value(&usage).unwrap();
        assert_eq!(json["prompt_tokens"], 100);
        assert_eq!(json["completion_tokens"], 50);
        assert_eq!(json["total_tokens"], 150);
    }

    #[test]
    fn test_text_generation_request_validation_empty_prompt() {
        use validator::Validate;
        let request = TextGenerationRequest {
            prompt: String::new(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 100,
            temperature: 0.7,
            provider: LlmProvider::OpenAI,
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_text_generation_request_validation_invalid_max_tokens() {
        use validator::Validate;
        let request = TextGenerationRequest {
            prompt: "test".to_string(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 5000, // exceeds max
            temperature: 0.7,
            provider: LlmProvider::OpenAI,
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_text_generation_request_validation_invalid_temperature() {
        use validator::Validate;
        let request = TextGenerationRequest {
            prompt: "test".to_string(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 100,
            temperature: 3.0, // exceeds max
            provider: LlmProvider::OpenAI,
        };
        assert!(request.validate().is_err());
    }
}