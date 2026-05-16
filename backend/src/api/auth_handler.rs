//! Shared auth handling for API endpoints.

use axum::http::HeaderMap;

use crate::api::state::AppState;
use crate::domain::auth::{AuthDomainError, AuthenticatedUser};
use crate::http_auth::bearer_token_from_headers;
use crate::models::AppError;

/// Convert domain auth errors into transport/API errors.
#[must_use]
pub fn map_auth_error(error: AuthDomainError) -> AppError {
    match error {
        AuthDomainError::Unauthorized(message) => AppError::Unauthorized(message),
        AuthDomainError::BadRequest(message) => AppError::BadRequest(message),
        AuthDomainError::ExternalService(message) => AppError::ExternalService(message),
        AuthDomainError::Configuration(message) => AppError::Configuration(message),
        AuthDomainError::Internal(message) => AppError::Internal(message),
    }
}

/// Resolve authenticated user from request headers through the shared auth use-case.
///
/// # Errors
///
/// Returns [`AppError`] when bearer extraction or auth lookup fails.
pub async fn authenticated_user_from_headers(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<AuthenticatedUser, AppError> {
    let token = bearer_token_from_headers(headers)?;
    state
        .auth_use_case
        .authenticate_with_bearer_token(&token)
        .await
        .map_err(map_auth_error)
}
