//! Shared helpers used across service modules.

use reqwest::Client;

use crate::models::AppError;

/// Build a default `reqwest` HTTP client.
///
/// # Errors
///
/// Returns [`AppError::Configuration`] if TLS or other client setup fails.
pub fn build_http_client() -> Result<Client, AppError> {
    Client::builder()
        .build()
        .map_err(|e| AppError::Configuration(format!("HTTP client error: {e}")))
}

/// Extract a non-empty API key from an optional slot.
///
/// # Errors
///
/// Returns [`AppError::Configuration`] when the key is `None` or empty.
pub fn require_api_key(slot: Option<&String>, var_name: &str) -> Result<String, AppError> {
    slot.filter(|k| !k.is_empty())
        .cloned()
        .ok_or_else(|| AppError::Configuration(format!("{var_name} not configured")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_api_key_returns_key_when_present() {
        let key = "sk-test".to_string();
        assert_eq!(require_api_key(Some(&key), "TEST_KEY").unwrap(), "sk-test");
    }

    #[test]
    fn require_api_key_errors_when_none() {
        let result = require_api_key(None, "TEST_KEY");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("TEST_KEY"));
    }

    #[test]
    fn require_api_key_errors_when_empty() {
        let empty = String::new();
        assert!(require_api_key(Some(&empty), "KEY").is_err());
    }

    #[test]
    fn build_http_client_succeeds() {
        assert!(build_http_client().is_ok());
    }
}
