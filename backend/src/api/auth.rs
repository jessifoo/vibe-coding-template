//! Authentication API endpoints.
//!
//! Handles user authentication and token management.

use axum::{
    Router,
    extract::{Json, State},
    http::HeaderMap,
    routing::{get, post},
};
use validator::Validate;

use crate::api::auth_handler::{authenticated_user_from_headers, map_auth_error};
use crate::api::state::AppState;
use crate::domain::auth::AuthenticatedUser;
use crate::models::{AppError, ProviderTokenRequest, TokenResponse, UserProfile};

/// Create the auth router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_current_user))
        .route("/provider-token", post(exchange_provider_token))
}

/// Get the currently authenticated user profile.
///
/// Requires a valid Bearer token in the Authorization header.
#[axum::debug_handler]
async fn get_current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserProfile>, AppError> {
    let user = authenticated_user_from_headers(&headers, &state).await?;

    tracing::info!(user_id = %user.id, "User authenticated");

    Ok(Json(to_api_user_profile(user)))
}

/// Exchange a provider token (Google, LinkedIn) for a Supabase token.
#[axum::debug_handler]
async fn exchange_provider_token(
    State(state): State<AppState>,
    Json(request): Json<ProviderTokenRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    request.validate().map_err(AppError::from)?;

    let access_token = state
        .auth_use_case
        .exchange_provider_token(&request.provider, &request.token)
        .await
        .map_err(map_auth_error)?;

    tracing::info!(provider = %request.provider, "Provider token exchanged");

    Ok(Json(TokenResponse::bearer(access_token)))
}

fn to_api_user_profile(user: AuthenticatedUser) -> UserProfile {
    UserProfile {
        id: user.id,
        email: user.email,
        full_name: user.full_name,
        avatar_url: user.avatar_url,
    }
}
