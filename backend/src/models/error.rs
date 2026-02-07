//! Application error types.
//!
//! Every fallible operation returns [`AppError`] which maps directly
//! to an HTTP status code and a JSON error body.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

/// JSON body returned for error responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Human-readable error message.
    pub error: String,

    /// HTTP status (serialisation skipped — sent via status code).
    #[serde(skip)]
    pub status_code: StatusCode,

    /// Optional extra detail (omitted from JSON when `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ApiErrorResponse {
    /// Create a new error response.
    pub fn new(status_code: StatusCode, error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            status_code,
            details: None,
        }
    }

    /// Attach additional detail.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        (self.status_code, Json(self)).into_response()
    }
}

// ---------------------------------------------------------------------------
// AppError
// ---------------------------------------------------------------------------

/// Unified application error.
#[derive(Debug, thiserror::Error)]
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
    /// Map each variant to the appropriate HTTP status code.
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
        let message = self.to_string();
        tracing::error!(status = %status_code, error = %message, "Request failed");
        ApiErrorResponse::new(status_code, message).into_response()
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_match_variants() {
        assert_eq!(
            AppError::Unauthorized("".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Forbidden("".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::NotFound("".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::BadRequest("".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::Validation("".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::ExternalService("".into()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            AppError::Configuration("".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            AppError::Internal("".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn display_messages_contain_context() {
        assert_eq!(
            AppError::Unauthorized("bad token".into()).to_string(),
            "Authentication failed: bad token"
        );
        assert_eq!(
            AppError::NotFound("user 1".into()).to_string(),
            "Resource not found: user 1"
        );
        assert_eq!(
            AppError::Validation("email".into()).to_string(),
            "Validation error: email"
        );
    }

    #[test]
    fn api_error_response_serialisation() {
        let resp = ApiErrorResponse::new(StatusCode::BAD_REQUEST, "oops");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\":\"oops\""));
        assert!(!json.contains("status_code"));
        assert!(!json.contains("details"));
    }

    #[test]
    fn api_error_response_with_details_serialisation() {
        let resp = ApiErrorResponse::new(StatusCode::BAD_REQUEST, "oops").with_details("extra");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"details\":\"extra\""));
    }

    #[test]
    fn from_serde_json_error() {
        let err: serde_json::Error = serde_json::from_str::<String>("bad").unwrap_err();
        let app_err = AppError::from(err);
        assert!(matches!(app_err, AppError::BadRequest(msg) if msg.contains("JSON")));
    }

    #[test]
    fn from_validator_errors() {
        use validator::Validate;

        #[derive(Validate)]
        struct T {
            #[validate(length(min = 5))]
            name: String,
        }

        let errs = T { name: "ab".into() }.validate().unwrap_err();
        let app_err = AppError::from(errs);
        assert!(matches!(app_err, AppError::Validation(_)));
    }
}
