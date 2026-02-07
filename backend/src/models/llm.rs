//! LLM request / response types.

use serde::{Deserialize, Serialize};
use validator::Validate;

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

/// Token-usage statistics returned by LLM APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    pub total_tokens: u32,
}

impl LlmUsage {
    pub const fn completion(prompt: u32, completion: u32) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: Some(completion),
            total_tokens: prompt + completion,
        }
    }

    pub const fn embedding(prompt: u32) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: None,
            total_tokens: prompt,
        }
    }
}

// ---------------------------------------------------------------------------
// Text generation
// ---------------------------------------------------------------------------

/// Request body for `/api/llm/generate`.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TextGenerationRequest {
    #[validate(length(min = 1, message = "Prompt cannot be empty"))]
    pub prompt: String,

    #[serde(default = "default_text_model")]
    pub model: String,

    #[serde(default = "default_max_tokens")]
    #[validate(range(min = 1, max = 4000, message = "max_tokens must be 1..4000"))]
    pub max_tokens: u32,

    #[serde(default = "default_temperature")]
    #[validate(range(min = 0.0, max = 2.0, message = "temperature must be 0.0..2.0"))]
    pub temperature: f32,

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

/// Response body for text generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextGenerationResponse {
    pub text: String,
    pub model: String,
    pub usage: LlmUsage,
}

// ---------------------------------------------------------------------------
// Embeddings
// ---------------------------------------------------------------------------

/// Request body for `/api/llm/embedding`.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct EmbeddingRequest {
    #[validate(length(min = 1, message = "Text cannot be empty"))]
    pub text: String,

    #[serde(default = "default_embedding_model")]
    pub model: String,

    #[serde(default)]
    pub provider: LlmProvider,
}

fn default_embedding_model() -> String {
    "text-embedding-ada-002".to_string()
}

/// Response body for embedding creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
    pub model: String,
    pub usage: LlmUsage,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- LlmUsage -----------------------------------------------------------

    #[test]
    fn usage_completion_totals() {
        let u = LlmUsage::completion(100, 50);
        assert_eq!(u.total_tokens, 150);
        assert_eq!(u.completion_tokens, Some(50));
    }

    #[test]
    fn usage_embedding_has_no_completion() {
        let u = LlmUsage::embedding(80);
        assert_eq!(u.total_tokens, 80);
        assert!(u.completion_tokens.is_none());
    }

    #[test]
    fn usage_embedding_omits_completion_in_json() {
        let json = serde_json::to_string(&LlmUsage::embedding(1)).unwrap();
        assert!(!json.contains("completion_tokens"));
    }

    // -- Provider -----------------------------------------------------------

    #[test]
    fn provider_default_is_openai() {
        assert_eq!(LlmProvider::default(), LlmProvider::OpenAI);
    }

    #[test]
    fn provider_serde_roundtrip() {
        for p in [LlmProvider::OpenAI, LlmProvider::Anthropic] {
            let s = serde_json::to_string(&p).unwrap();
            let back: LlmProvider = serde_json::from_str(&s).unwrap();
            assert_eq!(p, back);
        }
    }

    #[test]
    fn provider_display() {
        assert_eq!(LlmProvider::OpenAI.to_string(), "openai");
        assert_eq!(LlmProvider::Anthropic.to_string(), "anthropic");
    }

    // -- TextGenerationRequest ----------------------------------------------

    #[test]
    fn text_gen_request_applies_defaults() {
        let r: TextGenerationRequest = serde_json::from_str(r#"{"prompt":"hi"}"#).unwrap();
        assert_eq!(r.model, "gpt-3.5-turbo");
        assert_eq!(r.max_tokens, 500);
        assert!((r.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(r.provider, LlmProvider::OpenAI);
    }

    #[test]
    fn text_gen_request_validates_empty_prompt() {
        let r = TextGenerationRequest {
            prompt: String::new(),
            model: "m".into(),
            max_tokens: 10,
            temperature: 0.5,
            provider: LlmProvider::OpenAI,
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn text_gen_request_validates_max_tokens_range() {
        let r = TextGenerationRequest {
            prompt: "ok".into(),
            model: "m".into(),
            max_tokens: 5000,
            temperature: 0.5,
            provider: LlmProvider::OpenAI,
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn text_gen_request_validates_temperature_range() {
        let r = TextGenerationRequest {
            prompt: "ok".into(),
            model: "m".into(),
            max_tokens: 10,
            temperature: 3.0,
            provider: LlmProvider::OpenAI,
        };
        assert!(r.validate().is_err());
    }

    // -- EmbeddingRequest ---------------------------------------------------

    #[test]
    fn embedding_request_applies_defaults() {
        let r: EmbeddingRequest = serde_json::from_str(r#"{"text":"hi"}"#).unwrap();
        assert_eq!(r.model, "text-embedding-ada-002");
        assert_eq!(r.provider, LlmProvider::OpenAI);
    }

    #[test]
    fn embedding_request_rejects_empty_text() {
        let r = EmbeddingRequest {
            text: String::new(),
            model: "m".into(),
            provider: LlmProvider::OpenAI,
        };
        assert!(r.validate().is_err());
    }
}
