#![allow(clippy::unwrap_used)]
//! Integration test — health-check (ping) endpoint.
//!
//! Verifies the full application router responds correctly to `GET /`.
//!
//! **Requires** these environment variables to be set before running:
//! - `SUPABASE_URL`
//! - `SUPABASE_SERVICE_KEY`
//!
//! They are needed by the global `SETTINGS` lazy initialisation even though
//! the health-check endpoint itself does not use Supabase.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn returns_200_with_status_online() {
    let app = backend::create_app();

    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp).await;
    assert_eq!(json["status"], "online");
    assert!(json["version"].is_string());
    assert!(json["environment"].is_string());
}

#[tokio::test]
async fn returns_correct_environment() {
    let json = health_json().await;
    assert_eq!(json["environment"], "development");
}

#[tokio::test]
async fn returns_correct_version() {
    let json = health_json().await;
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let resp = backend::create_app()
        .oneshot(
            Request::builder()
                .uri("/no-such-route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn health_json() -> Value {
    let resp = backend::create_app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    body_json(resp).await
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
