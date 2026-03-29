//! API request and response models.
//!
//! All models use serde for serialization and validator for validation.
//! The Rust type system ensures these models are always valid at compile time.

pub mod auth;
pub mod error;
pub mod llm;
pub mod vectordb;

// Explicit re-exports from auth module
pub use auth::{ProviderTokenRequest, SupabaseUser, TokenResponse, UserMetadata, UserProfile};

// Explicit re-exports from error module
pub use error::{ApiErrorResponse, AppError};

// Explicit re-exports from llm module
pub use llm::{
    EmbeddingRequest, EmbeddingResponse, LlmProvider, LlmUsage, TextGenerationRequest,
    TextGenerationResponse,
};

// Explicit re-exports from vectordb module
pub use vectordb::{
    DeleteDocumentsRequest, Document, DocumentContent, DocumentInput, DocumentUploadResponse,
    SearchQuery, SearchResult,
};