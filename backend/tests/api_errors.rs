#![allow(clippy::unwrap_used)]
//! Integration tests — API error responses.
//!
//! These test that the full router returns the correct HTTP status codes
//! for common error conditions (missing auth, bad JSON, unknown routes).
//!
//! **Requires** `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` env vars.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::Value;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Auth endpoints — require a valid Bearer token
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_me_returns_401_without_token() {
    let resp = send(Request::builder().uri("/api/auth/me")).await;
    assert_eq!(resp.0, StatusCode::UNAUTHORIZED);
    assert!(resp.1["error"].as_str().unwrap().contains("Authorization"));
}

#[tokio::test]
async fn auth_me_returns_401_with_bad_scheme() {
    let resp = send(
        Request::builder()
            .uri("/api/auth/me")
            .header(header::AUTHORIZATION, "Basic bad"),
    )
    .await;
    assert_eq!(resp.0, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// LLM endpoints — require auth + valid JSON
// ---------------------------------------------------------------------------

#[tokio::test]
async fn llm_generate_returns_401_without_token() {
    let resp = send_json(
        Request::builder().method("POST").uri("/api/llm/generate"),
        r#"{"prompt":"hello"}"#,
    )
    .await;
    assert_eq!(resp.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn llm_embedding_returns_401_without_token() {
    let resp = send_json(
        Request::builder().method("POST").uri("/api/llm/embedding"),
        r#"{"text":"hello"}"#,
    )
    .await;
    assert_eq!(resp.0, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Vectordb endpoints — require auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn vectordb_search_returns_401_without_token() {
    let resp = send_json(
        Request::builder()
            .method("POST")
            .uri("/api/vectordb/search"),
        r#"{"query_text":"hello"}"#,
    )
    .await;
    assert_eq!(resp.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn vectordb_documents_returns_401_without_token() {
    let resp = send_json(
        Request::builder()
            .method("POST")
            .uri("/api/vectordb/documents"),
        r#"{"documents":[{"text":"hi"}]}"#,
    )
    .await;
    assert_eq!(resp.0, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Content-type / routing errors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn post_to_health_check_returns_405() {
    let resp = backend::create_app()
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
    let resp = send(Request::builder().uri("/api/nonexistent/path")).await;
    assert_eq!(resp.0, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn send(builder: axum::http::request::Builder) -> (StatusCode, Value) {
    let resp = backend::create_app()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn send_json(builder: axum::http::request::Builder, body: &str) -> (StatusCode, Value) {
    let resp = backend::create_app()
        .oneshot(
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}
