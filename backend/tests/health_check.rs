//! Integration test for the health check (ping) endpoint.
//!
//! This test starts the full application and verifies that the health check
//! endpoint at `/` responds correctly.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

/// Set up required environment variables for tests.
///
/// The SETTINGS lazy static requires SUPABASE_URL and SUPABASE_SERVICE_KEY
/// to be present, even though the health check doesn't use them.
fn setup_test_env() {
    // Only set if not already present to avoid overwriting real values
    if std::env::var("SUPABASE_URL").is_err() {
        std::env::set_var("SUPABASE_URL", "https://test.supabase.co");
    }
    if std::env::var("SUPABASE_SERVICE_KEY").is_err() {
        std::env::set_var("SUPABASE_SERVICE_KEY", "test-service-key");
    }
    if std::env::var("ENVIRONMENT").is_err() {
        std::env::set_var("ENVIRONMENT", "development");
    }
}

#[tokio::test]
async fn health_check_returns_200_with_status_online() {
    setup_test_env();

    let app = backend::create_app();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Read response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert_eq!(json["status"], "online");
    assert!(json["version"].is_string());
    assert!(json["environment"].is_string());
}

#[tokio::test]
async fn health_check_returns_correct_environment() {
    setup_test_env();

    let app = backend::create_app();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // In test environment, should be "development" (our default)
    assert_eq!(json["environment"], "development");
}

#[tokio::test]
async fn health_check_returns_correct_version() {
    setup_test_env();

    let app = backend::create_app();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Version should match Cargo.toml version
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_route_returns_404() {
    setup_test_env();

    let app = backend::create_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
