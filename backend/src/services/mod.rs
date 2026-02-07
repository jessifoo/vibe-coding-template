//! External-service integrations.
//!
//! Each sub-module owns a single external dependency:
//! - [`supabase`] — authentication, database, storage
//! - [`llm`] — text generation and embeddings (`OpenAI`, Anthropic)
//! - [`vectordb`] — semantic search (Qdrant)

pub mod llm;
pub mod supabase;
pub mod vectordb;
