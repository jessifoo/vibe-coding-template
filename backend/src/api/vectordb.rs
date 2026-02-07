//! Vector-database document storage and semantic search endpoints.

use axum::{
    Router,
    extract::Json,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use validator::Validate;

use crate::models::{
    AppError, DeleteDocumentsRequest, DocumentInput, DocumentUploadResponse, LlmProvider,
    SearchQuery, SearchResult,
};
use crate::services::llm::EmbeddingServiceFactory;
use crate::services::vectordb::qdrant::{DocumentData, QdrantService};
use crate::utils::{authenticate, truncate_for_log};

/// Vector-DB sub-router: `/api/vectordb/*`.
pub fn router() -> Router {
    Router::new()
        .route("/documents", post(add_documents).delete(delete_documents))
        .route("/search", post(search_documents))
}

/// Add documents to the vector database (creates embeddings automatically).
#[axum::debug_handler]
async fn add_documents(
    headers: HeaderMap,
    Json(request): Json<DocumentInput>,
) -> Result<Json<DocumentUploadResponse>, AppError> {
    let user = authenticate(&headers).await?;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        document_count = request.documents.len(),
        embedding_model = %request.embedding_model,
        "Adding documents to vector database",
    );

    let embedding_service = EmbeddingServiceFactory::create(LlmProvider::OpenAI)?;

    let mut embeddings = Vec::with_capacity(request.documents.len());
    for doc in &request.documents {
        let resp = embedding_service
            .create_embedding(&doc.text, &request.embedding_model)
            .await?;
        embeddings.push(resp.embedding);
    }

    let documents: Vec<DocumentData> = request
        .documents
        .iter()
        .map(|d| DocumentData {
            text: d.text.clone(),
            title: d.title.clone(),
        })
        .collect();

    let metadata: Vec<_> = request
        .documents
        .iter()
        .map(|d| d.metadata.clone())
        .collect();

    let vector_db = QdrantService::new()?;
    let document_ids = vector_db
        .add_documents(&documents, &embeddings, Some(&metadata))
        .await?;

    tracing::info!(user_id = %user.id, count = document_ids.len(), "Documents added");

    Ok(Json(DocumentUploadResponse { document_ids }))
}

/// Semantic search over stored documents.
#[axum::debug_handler]
async fn search_documents(
    headers: HeaderMap,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, AppError> {
    let user = authenticate(&headers).await?;
    query.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        query_preview = %truncate_for_log(&query.query_text, 50),
        limit = query.limit,
        "Searching vector database",
    );

    let embedding_service = EmbeddingServiceFactory::create(LlmProvider::OpenAI)?;
    let embedding = embedding_service
        .create_embedding(&query.query_text, &query.embedding_model)
        .await?;

    let vector_db = QdrantService::new()?;
    let results = vector_db
        .search(
            &embedding.embedding,
            query.limit,
            query.filter_metadata.as_ref(),
        )
        .await?;

    tracing::info!(user_id = %user.id, results = results.len(), "Search completed");

    Ok(Json(results))
}

/// Delete documents by ID.
#[axum::debug_handler]
async fn delete_documents(
    headers: HeaderMap,
    Json(request): Json<DeleteDocumentsRequest>,
) -> Result<StatusCode, AppError> {
    let user = authenticate(&headers).await?;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        count = request.document_ids.len(),
        "Deleting documents from vector database",
    );

    let vector_db = QdrantService::new()?;
    let deleted = vector_db.delete(&request.document_ids).await?;

    if deleted {
        tracing::info!(user_id = %user.id, count = request.document_ids.len(), "Documents deleted");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::BadRequest(
            "Failed to delete one or more documents".into(),
        ))
    }
}
