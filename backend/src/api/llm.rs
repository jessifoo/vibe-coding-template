//! LLM API endpoints.
//!
//! Handles text generation and embedding creation.

use axum::{
    extract::Json,
    http::{header, HeaderMap},
    routing::post,
    Router,
};

use crate::models::{
    AppError, EmbeddingRequest, EmbeddingResponse, LlmProvider, TextGenerationRequest,
    TextGenerationResponse,
};
use crate::services::llm::{EmbeddingServiceFactory, LlmServiceFactory};
use crate::services::supabase::SupabaseAuthService;

/// Create the LLM router.
pub fn router() -> Router {
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

    // Warn if using Anthropic (not supported)
    if request.provider == LlmProvider::Anthropic {
        return Err(AppError::BadRequest(
            "Anthropic does not support embeddings. Please use OpenAI (provider: 'openai')."
                .to_string(),
        ));
    }

    // Get embedding service
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

/// Truncate string for logging.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer test123".parse().unwrap());
        let token = extract_bearer_token(&headers).unwrap();
        assert_eq!(token, "test123");
    }

    #[test]
    fn test_extract_bearer_token_lowercase() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "bearer test123".parse().unwrap());
        let token = extract_bearer_token(&headers).unwrap();
        assert_eq!(token, "test123");
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        let headers = HeaderMap::new();
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn test_extract_bearer_token_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Basic abc".parse().unwrap());
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_truncate_empty_string() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_truncate_zero_length() {
        assert_eq!(truncate("hello", 0), "...");
    }
}
