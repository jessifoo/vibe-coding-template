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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer my_token".parse().unwrap());
        let token = extract_bearer_token(&headers).unwrap();
        assert_eq!(token, "my_token");
    }

    #[test]
    fn test_extract_bearer_token_lowercase() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "bearer my_token".parse().unwrap());
        let token = extract_bearer_token(&headers).unwrap();
        assert_eq!(token, "my_token");
    }

    #[test]
    fn test_extract_bearer_token_missing_header() {
        let headers = HeaderMap::new();
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Unauthorized(msg) => assert!(msg.contains("Missing")),
            other => panic!("Expected Unauthorized, got: {other:?}"),
        }
    }

    #[test]
    fn test_extract_bearer_token_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Basic abc123".parse().unwrap());
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Unauthorized(msg) => assert!(msg.contains("Invalid")),
            other => panic!("Expected Unauthorized, got: {other:?}"),
        }
    }

    #[test]
    fn test_extract_bearer_token_no_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "just_a_token".parse().unwrap());
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_bearer_token_empty_token() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer ".parse().unwrap());
        let token = extract_bearer_token(&headers).unwrap();
        assert_eq!(token, "");
    }
}
