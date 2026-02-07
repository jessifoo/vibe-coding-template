//! Shared utilities used across the API layer.
//!
//! Centralises common operations that were previously duplicated
//! in each endpoint module (violation of DRY).

use axum::http::{HeaderMap, header};

use crate::models::{AppError, UserProfile};
use crate::services::supabase::SupabaseAuthService;

/// Extract a Bearer token from the `Authorization` header.
///
/// Accepts both `Bearer ` and `bearer ` prefixes.
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] when the header is absent or malformed.
pub fn extract_bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".into()))?;

    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(String::from)
        .ok_or_else(|| {
            AppError::Unauthorized(
                "Invalid Authorization header format. Expected: Bearer <token>".into(),
            )
        })
}

/// Authenticate a request by verifying the Bearer token against Supabase.
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] if the token is missing or invalid.
pub async fn authenticate(headers: &HeaderMap) -> Result<UserProfile, AppError> {
    let token = extract_bearer_token(headers)?;
    let auth_service = SupabaseAuthService::new()?;
    auth_service.get_user(&token).await
}

/// Truncate a string for safe logging, appending `...` when shortened.
pub fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_bearer_token -----------------------------------------------

    #[test]
    fn bearer_token_extracted_with_standard_prefix() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer my_token".parse().unwrap());
        assert_eq!(extract_bearer_token(&h).unwrap(), "my_token");
    }

    #[test]
    fn bearer_token_extracted_with_lowercase_prefix() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "bearer my_token".parse().unwrap());
        assert_eq!(extract_bearer_token(&h).unwrap(), "my_token");
    }

    #[test]
    fn bearer_token_errors_when_header_missing() {
        let h = HeaderMap::new();
        let err = extract_bearer_token(&h).unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn bearer_token_errors_with_wrong_scheme() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Basic abc".parse().unwrap());
        assert!(extract_bearer_token(&h).is_err());
    }

    #[test]
    fn bearer_token_errors_with_no_prefix() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "raw_token".parse().unwrap());
        assert!(extract_bearer_token(&h).is_err());
    }

    #[test]
    fn bearer_token_allows_empty_token_value() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer ".parse().unwrap());
        assert_eq!(extract_bearer_token(&h).unwrap(), "");
    }

    // -- truncate_for_log ---------------------------------------------------

    #[test]
    fn truncate_returns_short_strings_unchanged() {
        assert_eq!(truncate_for_log("hi", 10), "hi");
    }

    #[test]
    fn truncate_returns_exact_length_unchanged() {
        assert_eq!(truncate_for_log("hello", 5), "hello");
    }

    #[test]
    fn truncate_shortens_long_strings() {
        assert_eq!(truncate_for_log("hello world", 5), "hello...");
    }

    #[test]
    fn truncate_handles_empty_string() {
        assert_eq!(truncate_for_log("", 5), "");
    }

    #[test]
    fn truncate_handles_zero_max() {
        assert_eq!(truncate_for_log("hello", 0), "...");
    }
}
