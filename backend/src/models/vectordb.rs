//! Vector database models.
//!
//! Types for document storage, retrieval, and semantic search.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;

/// A document to be stored in the vector database.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Document {
    /// Document text content
    #[validate(length(min = 1, message = "Document text cannot be empty"))]
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
    "text-embedding-ada-002".to_string()
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
    #[validate(length(min = 1, message = "Query text cannot be empty"))]
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
    use validator::Validate;

    #[test]
    fn test_document_validation_valid() {
        let doc = Document {
            text: "Hello world".to_string(),
            title: Some("Test".to_string()),
            metadata: HashMap::new(),
        };
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn test_document_validation_empty_text() {
        let doc = Document {
            text: String::new(),
            title: None,
            metadata: HashMap::new(),
        };
        assert!(doc.validate().is_err());
    }

    #[test]
    fn test_document_validation_no_title() {
        let doc = Document {
            text: "some text".to_string(),
            title: None,
            metadata: HashMap::new(),
        };
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            serde_json::Value::String("test".to_string()),
        );
        let doc = Document {
            text: "test".to_string(),
            title: None,
            metadata,
        };
        assert!(doc.validate().is_ok());
        assert_eq!(doc.metadata.len(), 1);
    }

    #[test]
    fn test_document_serialization_omits_null_title() {
        let doc = Document {
            text: "test".to_string(),
            title: None,
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&doc).unwrap();
        assert!(!json.contains("title"));
    }

    #[test]
    fn test_document_serialization_includes_title() {
        let doc = Document {
            text: "test".to_string(),
            title: Some("My Doc".to_string()),
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"title\":\"My Doc\""));
    }

    #[test]
    fn test_search_query_defaults() {
        let json = r#"{"query_text": "hello"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.limit, 10);
        assert_eq!(query.embedding_model, "text-embedding-ada-002");
        assert!(query.filter_metadata.is_none());
    }

    #[test]
    fn test_search_query_custom_values() {
        let json = r#"{"query_text":"test","limit":50,"embedding_model":"custom-model"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.query_text, "test");
        assert_eq!(query.limit, 50);
        assert_eq!(query.embedding_model, "custom-model");
    }

    #[test]
    fn test_search_query_validation_empty_text() {
        let query = SearchQuery {
            query_text: String::new(),
            embedding_model: "text-embedding-ada-002".to_string(),
            limit: 10,
            filter_metadata: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn test_search_query_validation_limit_too_high() {
        let query = SearchQuery {
            query_text: "test".to_string(),
            embedding_model: "text-embedding-ada-002".to_string(),
            limit: 200,
            filter_metadata: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn test_search_query_validation_limit_zero() {
        let query = SearchQuery {
            query_text: "test".to_string(),
            embedding_model: "text-embedding-ada-002".to_string(),
            limit: 0,
            filter_metadata: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn test_search_query_validation_valid() {
        let query = SearchQuery {
            query_text: "test".to_string(),
            embedding_model: "text-embedding-ada-002".to_string(),
            limit: 10,
            filter_metadata: None,
        };
        assert!(query.validate().is_ok());
    }

    #[test]
    fn test_document_input_validation_empty_docs() {
        let input = DocumentInput {
            documents: vec![],
            embedding_model: "text-embedding-ada-002".to_string(),
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_document_input_validation_valid() {
        let input = DocumentInput {
            documents: vec![Document {
                text: "hello".to_string(),
                title: None,
                metadata: HashMap::new(),
            }],
            embedding_model: "text-embedding-ada-002".to_string(),
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_document_input_defaults() {
        let json = r#"{"documents":[{"text":"hello"}]}"#;
        let input: DocumentInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.embedding_model, "text-embedding-ada-002");
        assert_eq!(input.documents.len(), 1);
    }

    #[test]
    fn test_delete_documents_request_validation_empty() {
        let req = DeleteDocumentsRequest {
            document_ids: vec![],
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_delete_documents_request_validation_valid() {
        let req = DeleteDocumentsRequest {
            document_ids: vec!["doc-1".to_string()],
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            id: "doc-1".to_string(),
            score: 0.95,
            document: DocumentContent {
                text: "hello world".to_string(),
                title: Some("Test".to_string()),
            },
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"id\":\"doc-1\""));
        assert!(json.contains("\"score\":0.95"));
        assert!(json.contains("\"text\":\"hello world\""));
    }

    #[test]
    fn test_document_content_omits_null_title() {
        let content = DocumentContent {
            text: "test".to_string(),
            title: None,
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(!json.contains("title"));
    }

    #[test]
    fn test_document_upload_response() {
        let resp = DocumentUploadResponse {
            document_ids: vec!["id1".to_string(), "id2".to_string()],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"document_ids\""));
        assert!(json.contains("\"id1\""));
        assert!(json.contains("\"id2\""));
    }
}
