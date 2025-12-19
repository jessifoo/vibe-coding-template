//! Authentication API endpoints.
//!
//! Handles user authentication and token management.

use axum::{
    extract::Json,
    http::{header, HeaderMap},
    routing::{get, post},
    Router,
};

use crate::models::{AppError, ProviderTokenRequest, TokenResponse, UserProfile};
use crate::services::supabase::SupabaseAuthService;

/// Create the auth router.
pub fn router() -> Router {
    Router::new()
        .route("/me", get(get_current_user))
        .route("/provider-token", post(exchange_provider_token))
}

/// Get the currently authenticated user profile.
///
/// Requires a valid Bearer token in the Authorization header.
#[axum::debug_handler]
async fn get_current_user(headers: HeaderMap) -> Result<Json<UserProfile>, AppError> {
    let token = extract_bearer_token(&headers)?;

    let auth_service = SupabaseAuthService::new()?;
    let user = auth_service.get_user(&token).await?;

    tracing::info!(user_id = %user.id, "User authenticated");

    Ok(Json(user))
}

/// Exchange a provider token (Google, LinkedIn) for a Supabase token.
#[axum::debug_handler]
async fn exchange_provider_token(
    Json(request): Json<ProviderTokenRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;

    let auth_service = SupabaseAuthService::new()?;
    let access_token = auth_service
        .sign_in_with_provider_token(&request.provider, &request.token)
        .await?;

    tracing::info!(provider = %request.provider, "Provider token exchanged");

    Ok(Json(TokenResponse::bearer(access_token)))
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
