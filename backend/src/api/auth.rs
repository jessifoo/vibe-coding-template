//! Authentication endpoints.

use axum::{
    Router,
    extract::Json,
    http::HeaderMap,
    routing::{get, post},
};
use validator::Validate;

use crate::models::{AppError, ProviderTokenRequest, TokenResponse, UserProfile};
use crate::services::supabase::SupabaseAuthService;
use crate::utils::extract_bearer_token;

/// Auth sub-router: `/api/auth/*`.
pub fn router() -> Router {
    Router::new()
        .route("/me", get(get_current_user))
        .route("/provider-token", post(exchange_provider_token))
}

/// Return the profile of the currently authenticated user.
#[axum::debug_handler]
async fn get_current_user(headers: HeaderMap) -> Result<Json<UserProfile>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let auth_service = SupabaseAuthService::new()?;
    let user = auth_service.get_user(&token).await?;

    tracing::info!(user_id = %user.id, "User authenticated");
    Ok(Json(user))
}

/// Exchange an OAuth provider token for a Supabase access token.
#[axum::debug_handler]
async fn exchange_provider_token(
    Json(request): Json<ProviderTokenRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    request.validate().map_err(AppError::from)?;

    let auth_service = SupabaseAuthService::new()?;
    let access_token = auth_service
        .sign_in_with_provider_token(&request.provider, &request.token)
        .await?;

    tracing::info!(provider = %request.provider, "Provider token exchanged");
    Ok(Json(TokenResponse::bearer(access_token)))
}
