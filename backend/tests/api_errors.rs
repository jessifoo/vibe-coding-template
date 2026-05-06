#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration tests — API error responses.
//!
//! These tests hit the full application router (built via `build_app()`)
//! and verify the correct HTTP status codes are returned for common
//! error conditions that should resolve without any external service:
//! - missing / malformed `Authorization` header
//! - unknown routes
//! - wrong method on an existing route
//!
//! **Requires** `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` env vars
//! (consumed by the global `SETTINGS` lazy initialisation during router
//! construction — the handlers themselves do not call Supabase for
//! these 4xx paths).

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use backend::app::build_app;
use serde_json::Value;
use tower::ServiceExt;

async fn app() -> Router {
    build_app()
        .await
        .expect("build_app() should succeed in tests")
}

async fn body_json(resp: Response) -> (StatusCode, Value) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn send(builder: axum::http::request::Builder) -> (StatusCode, Value) {
    let resp = app()
        .await
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    body_json(resp).await
}

async fn send_json(builder: axum::http::request::Builder, body: &str) -> (StatusCode, Value) {
    let resp = app()
        .await
        .oneshot(
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    body_json(resp).await
}

// ---------------------------------------------------------------------------
// Auth endpoints — require a valid Bearer token
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_me_returns_401_without_token() {
    let (status, json) = send(Request::builder().uri("/api/auth/me")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        json["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Authorization"),
        "expected error message to mention Authorization, got: {json}"
    );
}

#[tokio::test]
async fn auth_me_returns_401_with_wrong_scheme() {
    let (status, _) = send(
        Request::builder()
            .uri("/api/auth/me")
            .header(header::AUTHORIZATION, "Basic bad"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_me_returns_401_with_empty_bearer() {
    let (status, _) = send(
        Request::builder()
            .uri("/api/auth/me")
            .header(header::AUTHORIZATION, "Bearer   "),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// LLM endpoints — require auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn llm_generate_returns_401_without_token() {
    let (status, _) = send_json(
        Request::builder().method("POST").uri("/api/llm/generate"),
        r#"{"prompt":"hello"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn llm_embedding_returns_401_without_token() {
    let (status, _) = send_json(
        Request::builder().method("POST").uri("/api/llm/embedding"),
        r#"{"text":"hello"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Vectordb endpoints — require auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn vectordb_search_returns_401_without_token() {
    let (status, _) = send_json(
        Request::builder()
            .method("POST")
            .uri("/api/vectordb/search"),
        r#"{"query_text":"hello"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn vectordb_documents_returns_401_without_token() {
    let (status, _) = send_json(
        Request::builder()
            .method("POST")
            .uri("/api/vectordb/documents"),
        r#"{"documents":[{"text":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Routing errors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn post_to_health_check_returns_405() {
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn deep_unknown_route_returns_404() {
    let (status, _) = send(Request::builder().uri("/api/nonexistent/path")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
