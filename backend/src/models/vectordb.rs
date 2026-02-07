//! Vector-database document and search types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;

/// A document to store in the vector database.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Document {
    #[validate(length(min = 1, message = "Document text cannot be empty"))]
    pub text: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Batch-upload request.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct DocumentInput {
    #[validate(length(min = 1, message = "At least one document is required"))]
    #[validate(nested)]
    pub documents: Vec<Document>,

    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

fn default_embedding_model() -> String {
    "text-embedding-ada-002".to_string()
}

/// IDs returned after a successful upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUploadResponse {
    pub document_ids: Vec<String>,
}

/// Semantic-search query.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SearchQuery {
    #[validate(length(min = 1, message = "Query text cannot be empty"))]
    pub query_text: String,

    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,

    #[serde(default = "default_limit")]
    #[validate(range(min = 1, max = 100, message = "Limit must be 1..100"))]
    pub limit: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_metadata: Option<HashMap<String, serde_json::Value>>,
}

const fn default_limit() -> u32 {
    10
}

/// A single search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub document: DocumentContent,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// The text payload inside a search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Request to delete documents by ID.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct DeleteDocumentsRequest {
    #[validate(length(min = 1, message = "At least one document ID is required"))]
    pub document_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Document -----------------------------------------------------------

    #[test]
    fn valid_document_passes() {
        let d = Document {
            text: "hi".into(),
            title: None,
            metadata: HashMap::new(),
        };
        assert!(d.validate().is_ok());
    }

    #[test]
    fn empty_text_rejected() {
        let d = Document {
            text: String::new(),
            title: None,
            metadata: HashMap::new(),
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn document_omits_null_title() {
        let json = serde_json::to_string(&Document {
            text: "t".into(),
            title: None,
            metadata: HashMap::new(),
        })
        .unwrap();
        assert!(!json.contains("title"));
    }

    // -- DocumentInput ------------------------------------------------------

    #[test]
    fn document_input_defaults() {
        let i: DocumentInput = serde_json::from_str(r#"{"documents":[{"text":"hi"}]}"#).unwrap();
        assert_eq!(i.embedding_model, "text-embedding-ada-002");
    }

    #[test]
    fn empty_documents_rejected() {
        let i = DocumentInput {
            documents: vec![],
            embedding_model: "m".into(),
        };
        assert!(i.validate().is_err());
    }

    // -- SearchQuery --------------------------------------------------------

    #[test]
    fn search_query_defaults() {
        let q: SearchQuery = serde_json::from_str(r#"{"query_text":"hi"}"#).unwrap();
        assert_eq!(q.limit, 10);
        assert_eq!(q.embedding_model, "text-embedding-ada-002");
    }

    #[test]
    fn search_query_rejects_empty_text() {
        let q = SearchQuery {
            query_text: String::new(),
            embedding_model: "m".into(),
            limit: 10,
            filter_metadata: None,
        };
        assert!(q.validate().is_err());
    }

    #[test]
    fn search_query_rejects_limit_out_of_range() {
        for limit in [0, 200] {
            let q = SearchQuery {
                query_text: "ok".into(),
                embedding_model: "m".into(),
                limit,
                filter_metadata: None,
            };
            assert!(q.validate().is_err(), "limit {limit} should be rejected");
        }
    }

    // -- DeleteDocumentsRequest ---------------------------------------------

    #[test]
    fn delete_request_rejects_empty_ids() {
        let r = DeleteDocumentsRequest {
            document_ids: vec![],
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn delete_request_accepts_ids() {
        let r = DeleteDocumentsRequest {
            document_ids: vec!["x".into()],
        };
        assert!(r.validate().is_ok());
    }

    // -- SearchResult serialisation -----------------------------------------

    #[test]
    fn search_result_round_trips() {
        let r = SearchResult {
            id: "1".into(),
            score: 0.9,
            document: DocumentContent {
                text: "t".into(),
                title: None,
            },
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "1");
    }
}
