//! LLM API endpoints.
//!
//! Handles text generation and embedding creation.

use axum::{
    Router,
    extract::{Json, State},
    http::HeaderMap,
    routing::post,
};

use crate::api::auth_handler::authenticated_user_from_headers;
use crate::api::logging::{LOG_PREVIEW_CHARS, truncate_for_log};
use crate::api::state::AppState;
use crate::models::{
    AppError, EmbeddingRequest, EmbeddingResponse, TextGenerationRequest, TextGenerationResponse,
};
use crate::services::llm::{EmbeddingServiceFactory, LlmServiceFactory};

/// Create the LLM router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/generate", post(generate_text))
        .route("/embedding", post(create_embedding))
}

/// Generate text using the specified LLM model.
///
/// Requires authentication. Supports OpenAI and Anthropic providers.
#[axum::debug_handler]
async fn generate_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TextGenerationRequest>,
) -> Result<Json<TextGenerationResponse>, AppError> {
    let user = authenticated_user_from_headers(&headers, &state).await?;

    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        provider = %request.provider,
        model = %request.model,
        prompt_preview = %truncate_for_log(&request.prompt, LOG_PREVIEW_CHARS),
        "Text generation requested"
    );

    // Get LLM service for provider
    let llm_service = LlmServiceFactory::get_service(request.provider)?;

    // Generate text
    let response = llm_service
        .generate_text(
            &request.prompt,
            &request.model,
            request.max_tokens,
            request.temperature,
        )
        .await?;

    tracing::info!(
        user_id = %user.id,
        model = %response.model,
        tokens_used = response.usage.total_tokens,
        "Text generation completed"
    );

    Ok(Json(response))
}

/// Create an embedding vector for the provided text.
///
/// Requires authentication. Currently only supports OpenAI embeddings.
#[axum::debug_handler]
async fn create_embedding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, AppError> {
    let user = authenticated_user_from_headers(&headers, &state).await?;

    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        provider = %request.provider,
        model = %request.model,
        text_length = request.text.len(),
        "Embedding creation requested"
    );

    // Get embedding service (factory rejects Anthropic for embeddings)
    let embedding_service = EmbeddingServiceFactory::get_service(request.provider)?;

    // Create embedding
    let response = embedding_service
        .create_embedding(&request.text, &request.model)
        .await?;

    tracing::info!(
        user_id = %user.id,
        model = %response.model,
        embedding_dim = response.embedding.len(),
        "Embedding created"
    );

    Ok(Json(response))
}
