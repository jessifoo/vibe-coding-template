//! HTTP authentication helpers.
//!
//! Single implementation of Bearer token parsing from request headers, shared
//! by API layer and the Supabase auth service.

use axum::http::{header, HeaderMap};

use crate::models::AppError;

/// Return the token value from a raw `Authorization` header (without `Bearer `).
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] when the header is not a `Bearer` token or token is empty.
pub fn bearer_token_from_value(header: &str) -> Result<&str, AppError> {
    let (scheme, token) = header.split_once(' ').ok_or_else(|| {
        AppError::Unauthorized(
            "Invalid Authorization header format. Expected: Bearer <token>".to_string(),
        )
    })?;

    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(AppError::Unauthorized(
            "Invalid Authorization scheme. Expected: Bearer".to_string(),
        ));
    }

    if token.trim().is_empty() {
        return Err(AppError::Unauthorized(
            "Authorization token cannot be empty".to_string(),
        ));
    }

    Ok(token)
}

/// Extract a Bearer token from the request and return it as a `String` for
/// use with `SupabaseAuthService::get_user` and other APIs that need owned
/// data.
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] if the `Authorization` header is missing,
/// not UTF-8, or not a `Bearer` token.
pub fn bearer_token_from_headers(headers: &HeaderMap) -> Result<String, AppError> {
    let auth_header_value = headers
        .get(header::AUTHORIZATION)
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

    let auth_header = auth_header_value.to_str().map_err(|_| {
        AppError::Unauthorized("Invalid Authorization header encoding (non-UTF8)".to_string())
    })?;

    Ok(bearer_token_from_value(auth_header)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_token_from_value() {
        assert_eq!(
            bearer_token_from_value("Bearer test_token").expect("bearer token"),
            "test_token"
        );
        assert_eq!(
            bearer_token_from_value("bearer test_token").expect("bearer token"),
            "test_token"
        );
        assert!(bearer_token_from_value("Basic test").is_err());
        assert!(bearer_token_from_value("no_prefix").is_err());
    }
}