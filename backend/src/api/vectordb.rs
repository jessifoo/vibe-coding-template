//! Vector database API endpoints.
//!
//! Handles document storage and semantic search.

use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::api::state::AppState;
use crate::http_auth::bearer_token_from_headers;
use crate::models::{
    AppError, DeleteDocumentsRequest, Document, DocumentInput, DocumentUploadResponse, LlmProvider,
    SearchQuery, SearchResult,
};
use crate::services::llm::EmbeddingServiceFactory;
use crate::services::supabase::SupabaseAuthService;
use crate::services::vectordb::qdrant::DocumentData;

/// Create the vector database router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/documents", post(add_documents).delete(delete_documents))
        .route("/search", post(search_documents))
}

/// Add documents to the vector database.
///
/// Creates embeddings for each document and stores them in Qdrant.
#[axum::debug_handler]
async fn add_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DocumentInput>,
) -> Result<Json<DocumentUploadResponse>, AppError> {
    // Authenticate user
    let user = authenticate(&headers).await?;

    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        document_count = request.documents.len(),
        embedding_model = %request.embedding_model,
        "Adding documents to vector database"
    );

    // Get embedding service (OpenAI only for now)
    let embedding_service = EmbeddingServiceFactory::get_service(LlmProvider::OpenAI)?;

    // Generate embeddings for all documents
    let mut embeddings = Vec::with_capacity(request.documents.len());
    for doc in &request.documents {
        let embedding_response = embedding_service
            .create_embedding(&doc.text, &request.embedding_model)
            .await?;
        embeddings.push(embedding_response.embedding);
    }

    // Prepare documents for storage
    let documents: Vec<DocumentData> = request
        .documents
        .iter()
        .map(|d| DocumentData {
            text: d.text.clone(),
            title: d.title.clone(),
        })
        .collect();

    // Prepare metadata
    let metadata = user_scoped_metadata(&request.documents, &user.id);

    // Add to vector database
    let vector_db = &*state.qdrant;
    let document_ids = vector_db
        .add_documents(&documents, &embeddings, Some(&metadata))
        .await?;

    tracing::info!(
        user_id = %user.id,
        document_count = document_ids.len(),
        "Documents added to vector database"
    );

    Ok(Json(DocumentUploadResponse { document_ids }))
}

/// Search for documents similar to the query.
///
/// Creates an embedding for the query and performs semantic search.
#[axum::debug_handler]
async fn search_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, AppError> {
    // Authenticate user
    let user = authenticate(&headers).await?;

    // Validate request
    use validator::Validate;
    query.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        query_preview = %truncate(&query.query_text, 50),
        limit = query.limit,
        "Searching vector database"
    );

    // Get embedding service
    let embedding_service = EmbeddingServiceFactory::get_service(LlmProvider::OpenAI)?;

    // Create embedding for query
    let embedding_response = embedding_service
        .create_embedding(&query.query_text, &query.embedding_model)
        .await?;

    // Search vector database
    let vector_db = &*state.qdrant;
    let scoped_filter = user_scoped_filter(query.filter_metadata.as_ref(), &user.id);
    let results = vector_db
        .search(
            &embedding_response.embedding,
            query.limit,
            Some(&scoped_filter),
        )
        .await?;

    tracing::info!(
        user_id = %user.id,
        results_count = results.len(),
        "Search completed"
    );

    Ok(Json(results))
}

/// Delete documents from the vector database.
#[axum::debug_handler]
async fn delete_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteDocumentsRequest>,
) -> Result<StatusCode, AppError> {
    // Authenticate user
    let user = authenticate(&headers).await?;

    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        document_count = request.document_ids.len(),
        "Deleting documents from vector database"
    );

    // Delete from vector database
    let vector_db = &*state.qdrant;
    let success = vector_db
        .delete_for_user(&request.document_ids, &user.id)
        .await?;

    if success {
        tracing::info!(
            user_id = %user.id,
            document_count = request.document_ids.len(),
            "Documents deleted"
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::BadRequest(
            "Failed to delete one or more documents".to_string(),
        ))
    }
}

/// Authenticate user from request headers.
async fn authenticate(headers: &HeaderMap) -> Result<crate::models::UserProfile, AppError> {
    let token = bearer_token_from_headers(headers)?;
    let auth_service = SupabaseAuthService::new()?;
    auth_service.get_user(&token).await
}

/// Truncate string for logging (character-safe for UTF-8).
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Use character boundaries to avoid panics on multi-byte UTF-8
        s.chars().take(max_len).collect::<String>() + "..."
    }
}

fn user_scoped_metadata(documents: &[Document], user_id: &str) -> Vec<HashMap<String, JsonValue>> {
    documents
        .iter()
        .map(|document| {
            let mut metadata = document.metadata.clone();
            metadata.insert(
                "user_id".to_string(),
                JsonValue::String(user_id.to_string()),
            );
            metadata
        })
        .collect()
}

fn user_scoped_filter(
    filter_metadata: Option<&HashMap<String, JsonValue>>,
    user_id: &str,
) -> HashMap<String, JsonValue> {
    let mut scoped_filter = filter_metadata.cloned().unwrap_or_default();
    scoped_filter.insert(
        "user_id".to_string(),
        JsonValue::String(user_id.to_string()),
    );
    scoped_filter
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_scoped_filter_overrides_client_user_id() {
        let mut client_filter = HashMap::new();
        client_filter.insert("topic".to_string(), json!("billing"));
        client_filter.insert("user_id".to_string(), json!("attacker"));

        let scoped = user_scoped_filter(Some(&client_filter), "real-user");

        assert_eq!(scoped.get("topic"), Some(&json!("billing")));
        assert_eq!(scoped.get("user_id"), Some(&json!("real-user")));
    }

    #[test]
    fn user_scoped_metadata_injects_owner_id() {
        let mut metadata = HashMap::new();
        metadata.insert("team".to_string(), json!("search"));

        let documents = vec![Document {
            text: "internal notes".to_string(),
            title: Some("ops".to_string()),
            metadata,
        }];

        let scoped = user_scoped_metadata(&documents, "user-123");
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].get("team"), Some(&json!("search")));
        assert_eq!(scoped[0].get("user_id"), Some(&json!("user-123")));
    }
}
