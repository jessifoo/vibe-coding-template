//! LLM services — text generation and embeddings.

pub mod embedding;
#[allow(clippy::module_inception)]
pub mod llm;

pub use embedding::{EmbeddingService, EmbeddingServiceFactory};
pub use llm::{LlmService, LlmServiceFactory};
