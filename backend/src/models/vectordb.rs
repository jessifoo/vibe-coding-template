//! Vector database models.
//!
//! Types for document storage, retrieval, and semantic search.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::{Validate, ValidationError};

/// Reject empty or whitespace-only text (stricter than `length(min = 1)`).
fn validate_trim_non_empty(s: &str) -> Result<(), ValidationError> {
    if s.trim().is_empty() {
        return Err(ValidationError::new("trim_non_empty"));
    }
    Ok(())
}

/// A document to be stored in the vector database.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Document {
    /// Document text content
    #[validate(custom(function = "validate_trim_non_empty", message = "Document text cannot be empty or whitespace only"))]
    pub text: String,

    /// Optional document title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Optional metadata for filtering and retrieval
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Input for adding documents to the vector database.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct DocumentInput {
    /// Documents to add
    #[validate(length(min = 1, message = "At least one document is required"))]
    #[validate(nested)]
    pub documents: Vec<Document>,

    /// Embedding model to use
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

/// Response from adding documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUploadResponse {
    /// IDs of the uploaded documents
    pub document_ids: Vec<String>,
}

/// Query for searching the vector database.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SearchQuery {
    /// Text to search for
    #[validate(custom(function = "validate_trim_non_empty", message = "Query text cannot be empty or whitespace only"))]
    pub query_text: String,

    /// Embedding model to use
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    pub limit: u32,

    /// Optional metadata filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_metadata: Option<HashMap<String, serde_json::Value>>,
}

const fn default_limit() -> u32 {
    10
}

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Document ID
    pub id: String,

    /// Similarity score (0.0-1.0)
    pub score: f32,

    /// The document content
    pub document: DocumentContent,

    /// Document metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Document content from search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    /// Document text
    pub text: String,

    /// Document title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Request to delete documents.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct DeleteDocumentsRequest {
    /// IDs of documents to delete
    #[validate(length(min = 1, message = "At least one document ID is required"))]
    pub document_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_validation() {
        let valid_doc = Document {
            text: "Hello world".to_string(),
            title: Some("Test".to_string()),
            metadata: HashMap::new(),
        };
        assert!(valid_doc.validate().is_ok());

        let invalid_doc = Document {
            text: String::new(),
            title: None,
            metadata: HashMap::new(),
        };
        assert!(invalid_doc.validate().is_err());

        let whitespace_doc = Document {
            text: "   \t  ".to_string(),
            title: None,
            metadata: HashMap::new(),
        };
        assert!(whitespace_doc.validate().is_err());
    }

    #[test]
    fn test_search_query_defaults() {
        let json = r#"{"query_text": "hello"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.limit, 10);
        assert_eq!(query.embedding_model, "text-embedding-3-small");
    }
}
