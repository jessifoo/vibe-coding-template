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

/// Fatal errors when building or starting the HTTP server (outside request handling).
#[derive(Debug, Error)]
pub enum AppRunError {
    #[error("Failed to initialize Qdrant: {0}")]
    QdrantInit(AppError),

    #[error("Server bind or accept error: {0}")]
    Io(#[from] std::io::Error),
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

        // Log 4xx errors at WARN, 5xx errors at ERROR
        if status_code.is_client_error() {
            tracing::warn!(
                status_code = %status_code,
                error = %error_message,
                "Client error"
            );
        } else {
            tracing::error!(
                status_code = %status_code,
                error = %error_message,
                "Server error"
            );
        }

        ApiErrorResponse::new(status_code, error_message).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            return Self::ExternalService(format!("HTTP client timeout: {err}"));
        }
        if err.is_connect() {
            return Self::ExternalService(format!("HTTP connect error: {err}"));
        }
        if let Some(status) = err.status() {
            let target = err
                .url()
                .map(|u| u.as_str().to_string())
                .unwrap_or_default();
            return Self::ExternalService(format!(
                "HTTP error (status {status}): {err}; url={target}"
            ));
        }
        Self::ExternalService(format!("HTTP request error: {err}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::Internal(format!("JSON error: {err}"))
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        Self::Validation(err.to_string())
    }
}
