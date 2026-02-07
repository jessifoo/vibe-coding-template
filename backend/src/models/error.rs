//! Error types and API error responses.
//!
//! Provides consistent error handling across the API with proper HTTP status codes.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API error response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Error message
    pub error: String,

    /// HTTP status code
    #[serde(skip)]
    pub status_code: StatusCode,

    /// Optional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ApiErrorResponse {
    /// Create a new API error response.
    #[must_use]
    pub fn new(status_code: StatusCode, error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            status_code,
            details: None,
        }
    }

    /// Add details to the error response.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        let status = self.status_code;
        (status, Json(self)).into_response()
    }
}

/// Application error type.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    #[error("Access denied: {0}")]
    Forbidden(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl AppError {
    /// Get the HTTP status code for this error.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::ExternalService(_) => StatusCode::BAD_GATEWAY,
            Self::Configuration(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        let error_message = self.to_string();

        tracing::error!(
            status_code = %status_code,
            error = %error_message,
            "Request failed"
        );

        ApiErrorResponse::new(status_code, error_message).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        Self::ExternalService(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::BadRequest(format!("JSON error: {err}"))
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        Self::Validation(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_status_codes() {
        assert_eq!(
            AppError::Unauthorized("test".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Forbidden("test".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::NotFound("test".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::BadRequest("test".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::Validation("test".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::ExternalService("test".into()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            AppError::Configuration("test".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            AppError::Internal("test".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_app_error_display() {
        let err = AppError::Unauthorized("invalid token".into());
        assert_eq!(err.to_string(), "Authentication failed: invalid token");

        let err = AppError::NotFound("user 123".into());
        assert_eq!(err.to_string(), "Resource not found: user 123");

        let err = AppError::BadRequest("missing field".into());
        assert_eq!(err.to_string(), "Invalid request: missing field");

        let err = AppError::ExternalService("timeout".into());
        assert_eq!(err.to_string(), "External service error: timeout");

        let err = AppError::Configuration("missing key".into());
        assert_eq!(err.to_string(), "Configuration error: missing key");

        let err = AppError::Internal("unexpected".into());
        assert_eq!(err.to_string(), "Internal server error: unexpected");

        let err = AppError::Forbidden("access denied".into());
        assert_eq!(err.to_string(), "Access denied: access denied");

        let err = AppError::Validation("email invalid".into());
        assert_eq!(err.to_string(), "Validation error: email invalid");
    }

    #[test]
    fn test_api_error_response_new() {
        let resp = ApiErrorResponse::new(StatusCode::BAD_REQUEST, "test error");
        assert_eq!(resp.error, "test error");
        assert_eq!(resp.status_code, StatusCode::BAD_REQUEST);
        assert!(resp.details.is_none());
    }

    #[test]
    fn test_api_error_response_with_details() {
        let resp =
            ApiErrorResponse::new(StatusCode::BAD_REQUEST, "test error").with_details("more info");
        assert_eq!(resp.error, "test error");
        assert_eq!(resp.details, Some("more info".to_string()));
    }

    #[test]
    fn test_api_error_response_serialization() {
        let resp = ApiErrorResponse::new(StatusCode::BAD_REQUEST, "test error");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\":\"test error\""));
        // status_code is skip, should not appear
        assert!(!json.contains("status_code"));
    }

    #[test]
    fn test_api_error_response_details_omitted_when_none() {
        let resp = ApiErrorResponse::new(StatusCode::BAD_REQUEST, "test");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("details"));
    }

    #[test]
    fn test_api_error_response_details_included_when_some() {
        let resp =
            ApiErrorResponse::new(StatusCode::BAD_REQUEST, "test").with_details("extra info");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"details\":\"extra info\""));
    }

    #[test]
    fn test_from_serde_json_error() {
        let err: serde_json::Error = serde_json::from_str::<String>("not valid json").unwrap_err();
        let app_err = AppError::from(err);
        match app_err {
            AppError::BadRequest(msg) => assert!(msg.contains("JSON error")),
            other => panic!("Expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn test_from_validator_errors() {
        use validator::Validate;

        #[derive(validator::Validate)]
        struct TestStruct {
            #[validate(length(min = 5))]
            name: String,
        }

        let item = TestStruct {
            name: "ab".to_string(),
        };
        let validation_result = item.validate();
        assert!(validation_result.is_err());

        let app_err = AppError::from(validation_result.unwrap_err());
        match app_err {
            AppError::Validation(msg) => assert!(!msg.is_empty()),
            other => panic!("Expected Validation, got: {other:?}"),
        }
    }
}
