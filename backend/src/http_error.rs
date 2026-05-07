//! Shared helpers for HTTP client error handling (one place for body reads on failed responses).
//!
//! Use [`read_failed_response_text`] when handling non-success `reqwest::Response` values so
//! body-read errors are not silently dropped.

use reqwest::Response;
use reqwest::StatusCode;

use crate::models::AppError;

/// Read the body of a **non-success** HTTP response.
///
/// If `response.text()` fails, returns [`AppError::ExternalService`] that includes the HTTP
/// status and the body-read error (the upstream error is not swallowed).
///
/// # Errors
///
/// Returns [`AppError::ExternalService`] when the response body cannot be read.
pub async fn read_failed_response_text(
    response: Response,
) -> Result<(StatusCode, String), AppError> {
    let status = response.status();
    let text = response.text().await.map_err(|e| {
        AppError::ExternalService(format!(
            "HTTP error response (status {status}): failed to read body: {e}"
        ))
    })?;
    Ok((status, text))
}
