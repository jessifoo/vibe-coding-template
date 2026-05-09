#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration tests — health-check (`GET /`) endpoint.
//!
//! Exercises the full application router (including middleware and
//! shared [`AppState`]) to verify the liveness endpoint stays honest.
//!
//! **Requires** these environment variables to be set before running
//! (they are consumed by the global `SETTINGS` lazy initialisation):
//! - `SUPABASE_URL`
//! - `SUPABASE_SERVICE_KEY`

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use backend::app::build_app;
use serde_json::Value;
use tower::ServiceExt;

async fn app() -> Router {
    build_app()
        .await
        .expect("build_app() should succeed in tests")
}

async fn body_json(resp: Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn health_json() -> Value {
    let resp = app()
        .await
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    body_json(resp).await
}

#[tokio::test]
async fn returns_200_with_status_online() {
    let resp = app()
        .await
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
async fn returns_crate_version() {
    let json = health_json().await;
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn returns_an_environment_string() {
    let json = health_json().await;
    assert!(
        json["environment"].is_string(),
        "environment should be a string"
    );
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let resp = app()
        .await
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
