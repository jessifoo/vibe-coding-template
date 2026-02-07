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
    use validator::Validate;

    #[test]
    fn test_llm_usage_completion() {
        let usage = LlmUsage::completion(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, Some(50));
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_llm_usage_completion_zero() {
        let usage = LlmUsage::completion(0, 0);
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, Some(0));
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_llm_usage_embedding() {
        let usage = LlmUsage::embedding(100);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, None);
        assert_eq!(usage.total_tokens, 100);
    }

    #[test]
    fn test_llm_usage_serialization_completion() {
        let usage = LlmUsage::completion(10, 20);
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"prompt_tokens\":10"));
        assert!(json.contains("\"completion_tokens\":20"));
        assert!(json.contains("\"total_tokens\":30"));
    }

    #[test]
    fn test_llm_usage_serialization_embedding_omits_completion() {
        let usage = LlmUsage::embedding(50);
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"prompt_tokens\":50"));
        assert!(!json.contains("completion_tokens"));
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(LlmProvider::OpenAI.to_string(), "openai");
        assert_eq!(LlmProvider::Anthropic.to_string(), "anthropic");
    }

    #[test]
    fn test_provider_default() {
        let provider = LlmProvider::default();
        assert_eq!(provider, LlmProvider::OpenAI);
    }

    #[test]
    fn test_provider_serialization() {
        let json = serde_json::to_string(&LlmProvider::OpenAI).unwrap();
        assert_eq!(json, "\"openai\"");

        let json = serde_json::to_string(&LlmProvider::Anthropic).unwrap();
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
    fn test_text_generation_request_defaults() {
        let json = r#"{"prompt":"hello"}"#;
        let req: TextGenerationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.prompt, "hello");
        assert_eq!(req.model, "gpt-3.5-turbo");
        assert_eq!(req.max_tokens, 500);
        assert!((req.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(req.provider, LlmProvider::OpenAI);
    }

    #[test]
    fn test_text_generation_request_custom_values() {
        let json = r#"{"prompt":"test","model":"gpt-4","max_tokens":1000,"temperature":0.5,"provider":"anthropic"}"#;
        let req: TextGenerationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.prompt, "test");
        assert_eq!(req.model, "gpt-4");
        assert_eq!(req.max_tokens, 1000);
        assert!((req.temperature - 0.5).abs() < f32::EPSILON);
        assert_eq!(req.provider, LlmProvider::Anthropic);
    }

    #[test]
    fn test_text_generation_request_validation_empty_prompt() {
        let req = TextGenerationRequest {
            prompt: String::new(),
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: 500,
            temperature: 0.7,
            provider: LlmProvider::OpenAI,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_text_generation_request_validation_max_tokens_too_high() {
        let req = TextGenerationRequest {
            prompt: "hello".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: 5000,
            temperature: 0.7,
            provider: LlmProvider::OpenAI,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_text_generation_request_validation_temperature_out_of_range() {
        let req = TextGenerationRequest {
            prompt: "hello".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: 500,
            temperature: 3.0,
            provider: LlmProvider::OpenAI,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_text_generation_request_validation_valid() {
        let req = TextGenerationRequest {
            prompt: "hello".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: 500,
            temperature: 0.7,
            provider: LlmProvider::OpenAI,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_embedding_request_defaults() {
        let json = r#"{"text":"hello world"}"#;
        let req: EmbeddingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.text, "hello world");
        assert_eq!(req.model, "text-embedding-ada-002");
        assert_eq!(req.provider, LlmProvider::OpenAI);
    }

    #[test]
    fn test_embedding_request_validation_empty_text() {
        let req = EmbeddingRequest {
            text: String::new(),
            model: "text-embedding-ada-002".to_string(),
            provider: LlmProvider::OpenAI,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_embedding_request_validation_valid() {
        let req = EmbeddingRequest {
            text: "hello".to_string(),
            model: "text-embedding-ada-002".to_string(),
            provider: LlmProvider::OpenAI,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_text_generation_response_serialization() {
        let resp = TextGenerationResponse {
            text: "Generated text".to_string(),
            model: "gpt-4".to_string(),
            usage: LlmUsage::completion(10, 20),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"text\":\"Generated text\""));
        assert!(json.contains("\"model\":\"gpt-4\""));
    }

    #[test]
    fn test_embedding_response_serialization() {
        let resp = EmbeddingResponse {
            embedding: vec![0.1, 0.2, 0.3],
            model: "text-embedding-ada-002".to_string(),
            usage: LlmUsage::embedding(5),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"model\":\"text-embedding-ada-002\""));
        assert!(json.contains("\"embedding\""));
    }

    #[test]
    fn test_provider_equality() {
        assert_eq!(LlmProvider::OpenAI, LlmProvider::OpenAI);
        assert_ne!(LlmProvider::OpenAI, LlmProvider::Anthropic);
    }

    #[test]
    fn test_provider_copy() {
        let p1 = LlmProvider::OpenAI;
        let p2 = p1;
        assert_eq!(p1, p2);
    }
}
