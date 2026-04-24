//! Application services module.
//!
//! Contains all external service integrations including:
//! - Supabase (auth, database, storage)
//! - LLM providers (OpenAI, Anthropic)
//! - Vector database (Qdrant)

pub mod llm;
pub mod supabase;
pub mod vectordb;
