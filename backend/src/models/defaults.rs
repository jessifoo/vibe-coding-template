//! Shared model defaults used across API request structs.
//!
//! Keeping defaults centralized prevents accidental drift between endpoints that
//! should use the same model baseline.

/// Default embedding model for vector and embedding requests.
#[must_use]
pub fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}
