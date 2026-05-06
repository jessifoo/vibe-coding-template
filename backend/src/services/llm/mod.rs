//! LLM (Large Language Model) services.
//!
//! Provides text generation and embedding services via OpenAI and Anthropic.

pub mod embedding;
pub mod text_generation;

pub use embedding::{EmbeddingService, EmbeddingServiceFactory};
pub use text_generation::{LlmService, LlmServiceFactory};
