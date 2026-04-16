//! Vector database API endpoints.
//!
//! Handles document storage and semantic search.

use axum::{
    extract::Json,
    http::{header, HeaderMap, StatusCode},
    routing::post,
    Router,
};

use crate::models::{
    AppError, DeleteDocumentsRequest, DocumentInput, DocumentUploadResponse, LlmProvider,
    SearchQuery, SearchResult,
};
use crate::services::llm::EmbeddingServiceFactory;
use crate::services::supabase::SupabaseAuthService;
use crate::services::vectordb::qdrant::{DocumentData, QdrantService, OWNER_USER_ID_METADATA_KEY};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Create the vector database router.
pub fn router() -> Router {
    Router::new()
        .route("/documents", post(add_documents).delete(delete_documents))
        .route("/search", post(search_documents))
}

/// Add documents to the vector database.
///
/// Creates embeddings for each document and stores them in Qdrant.
#[axum::debug_handler]
async fn add_documents(
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
    let metadata: Vec<_> = request
        .documents
        .iter()
        .map(|d| with_owner_metadata(d.metadata.clone(), &user.id))
        .collect();

    // Add to vector database
    let vector_db = QdrantService::new().await?;
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

    // Enforce tenant isolation regardless of any user-provided filters.
    let filter_metadata = build_owner_scoped_filter(query.filter_metadata.as_ref(), &user.id);

    // Search vector database
    let vector_db = QdrantService::new().await?;
    let results = vector_db
        .search(
            &embedding_response.embedding,
            query.limit,
            Some(&filter_metadata),
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
    let vector_db = QdrantService::new().await?;
    let success = vector_db.delete(&request.document_ids, &user.id).await?;

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
    let token = extract_bearer_token(headers)?;
    let auth_service = SupabaseAuthService::new()?;
    auth_service.get_user(&token).await
}

/// Extract bearer token from Authorization header.
fn extract_bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

    auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
        .map(|s| s.to_string())
        .ok_or_else(|| {
            AppError::Unauthorized(
                "Invalid Authorization header format. Expected: Bearer <token>".to_string(),
            )
        })
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

fn with_owner_metadata(
    mut metadata: HashMap<String, JsonValue>,
    user_id: &str,
) -> HashMap<String, JsonValue> {
    metadata.insert(
        OWNER_USER_ID_METADATA_KEY.to_string(),
        JsonValue::String(user_id.to_string()),
    );
    metadata
}

fn build_owner_scoped_filter(
    filter_metadata: Option<&HashMap<String, JsonValue>>,
    user_id: &str,
) -> HashMap<String, JsonValue> {
    let mut scoped_filters = filter_metadata.cloned().unwrap_or_default();
    scoped_filters.insert(
        OWNER_USER_ID_METADATA_KEY.to_string(),
        JsonValue::String(user_id.to_string()),
    );
    scoped_filters
}

#[cfg(test)]
mod tests {
    use super::{build_owner_scoped_filter, with_owner_metadata, OWNER_USER_ID_METADATA_KEY};
    use serde_json::Value as JsonValue;
    use std::collections::HashMap;

    #[test]
    fn with_owner_metadata_overrides_spoofed_owner() {
        let mut input = HashMap::new();
        input.insert(
            OWNER_USER_ID_METADATA_KEY.to_string(),
            JsonValue::String("attacker-id".to_string()),
        );
        input.insert(
            "topic".to_string(),
            JsonValue::String("security".to_string()),
        );

        let output = with_owner_metadata(input, "real-user-id");

        assert_eq!(
            output.get(OWNER_USER_ID_METADATA_KEY),
            Some(&JsonValue::String("real-user-id".to_string()))
        );
        assert_eq!(
            output.get("topic"),
            Some(&JsonValue::String("security".to_string()))
        );
    }

    #[test]
    fn build_owner_scoped_filter_enforces_authenticated_user() {
        let mut user_filters = HashMap::new();
        user_filters.insert(
            OWNER_USER_ID_METADATA_KEY.to_string(),
            JsonValue::String("attacker-id".to_string()),
        );
        user_filters.insert(
            "project".to_string(),
            JsonValue::String("alpha".to_string()),
        );

        let scoped = build_owner_scoped_filter(Some(&user_filters), "auth-user-id");

        assert_eq!(
            scoped.get(OWNER_USER_ID_METADATA_KEY),
            Some(&JsonValue::String("auth-user-id".to_string()))
        );
        assert_eq!(
            scoped.get("project"),
            Some(&JsonValue::String("alpha".to_string()))
        );
    }
}
