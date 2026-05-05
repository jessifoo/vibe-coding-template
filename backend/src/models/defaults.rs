//! Shared `#[serde(default = "...")]` helpers for model modules.

/// Default OpenAI embedding model (matches `EmbeddingRequest` in `llm`).
#[must_use]
pub fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}
