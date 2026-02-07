//! LLM text-generation and embedding endpoints.

use axum::{Router, extract::Json, http::HeaderMap, routing::post};
use validator::Validate;

use crate::models::{
    AppError, EmbeddingRequest, EmbeddingResponse, LlmProvider, TextGenerationRequest,
    TextGenerationResponse,
};
use crate::services::llm::{EmbeddingServiceFactory, LlmServiceFactory};
use crate::utils::{authenticate, truncate_for_log};

/// LLM sub-router: `/api/llm/*`.
pub fn router() -> Router {
    Router::new()
        .route("/generate", post(generate_text))
        .route("/embedding", post(create_embedding))
}

/// Generate text using the specified LLM model.
#[axum::debug_handler]
async fn generate_text(
    headers: HeaderMap,
    Json(request): Json<TextGenerationRequest>,
) -> Result<Json<TextGenerationResponse>, AppError> {
    let user = authenticate(&headers).await?;
    request.validate().map_err(AppError::from)?;

    tracing::info!(
        user_id = %user.id,
        provider = %request.provider,
        model = %request.model,
        prompt_preview = %truncate_for_log(&request.prompt, 50),
        "Text generation requested",
    );

    let service = LlmServiceFactory::create(request.provider)?;
    let response = service
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
        "Text generation completed",
    );

    Ok(Json(response))
}

/// Create an embedding vector for the given text.
#[axum::debug_handler]
async fn create_embedding(
    headers: HeaderMap,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, AppError> {
    let user = authenticate(&headers).await?;
    request.validate().map_err(AppError::from)?;

    if request.provider == LlmProvider::Anthropic {
        return Err(AppError::BadRequest(
            "Anthropic does not support embeddings. Use provider 'openai' instead.".into(),
        ));
    }

    tracing::info!(
        user_id = %user.id,
        provider = %request.provider,
        model = %request.model,
        text_length = request.text.len(),
        "Embedding creation requested",
    );

    let service = EmbeddingServiceFactory::create(request.provider)?;
    let response = service
        .create_embedding(&request.text, &request.model)
        .await?;

    tracing::info!(
        user_id = %user.id,
        model = %response.model,
        embedding_dim = response.embedding.len(),
        "Embedding created",
    );

    Ok(Json(response))
}
