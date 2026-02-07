//! LLM (Large Language Model) services.
//!
//! Provides text generation and embedding services via OpenAI and Anthropic.

pub mod embedding;
#[allow(clippy::module_inception)]
pub mod llm;

pub use embedding::{EmbeddingService, EmbeddingServiceFactory};
pub use llm::{LlmService, LlmServiceFactory};
