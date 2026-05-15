//! LLM API endpoints.
//!
//! Handles text generation and embedding creation.

use axum::{Router, extract::Json, http::HeaderMap, routing::post};

use crate::api::state::AppState;
use crate::http_auth::bearer_token_from_headers;
use crate::models::{
    AppError, EmbeddingRequest, EmbeddingResponse, TextGenerationRequest, TextGenerationResponse,
};
use crate::services::llm::{EmbeddingServiceFactory, LlmServiceFactory};
use crate::services::supabase::SupabaseAuthService;

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
    headers: HeaderMap,
    Json(request): Json<TextGenerationRequest>,
) -> Result<Json<TextGenerationResponse>, AppError> {
    // Authenticate user
    let user = authenticate(&headers).await?;

    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        provider = %request.provider,
        model = %request.model,
        prompt_preview = %truncate(&request.prompt, 50),
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
    headers: HeaderMap,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, AppError> {
    // Authenticate user
    let user = authenticate(&headers).await?;

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
