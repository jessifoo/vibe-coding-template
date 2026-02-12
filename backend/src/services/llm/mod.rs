//! LLM services — text generation and embeddings.
//!
//! Uses trait-based abstractions (`async-trait`) so each provider is:
//! - independently testable / mockable,
//! - swappable at runtime (Open/Closed Principle),
//! - behind a stable interface (Dependency Inversion).

pub mod embedding;
#[allow(clippy::module_inception)]
pub mod llm;

pub use embedding::{EmbeddingService, EmbeddingServiceFactory};
pub use llm::{LlmService, LlmServiceFactory};
