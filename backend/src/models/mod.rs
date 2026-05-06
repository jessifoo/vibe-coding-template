//! API request and response models.
//!
//! All models use serde for serialization and the `validator` crate for
//! runtime checks at API boundaries. Strong types catch many mistakes at
//! compile time; rules like string length and formats are still enforced when
//! requests are validated.

pub mod auth;
pub mod defaults;
pub mod error;
pub mod llm;
pub mod vectordb;

// Explicit re-exports from auth module
pub use auth::{ProviderTokenRequest, SupabaseUser, TokenResponse, UserMetadata, UserProfile};

// Explicit re-exports from error module
pub use error::{ApiErrorResponse, AppError, AppRunError};

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