//! Full Stack Application Backend
//!
//! A Rust backend built with Axum, featuring:
//! - Supabase authentication and database integration
//! - LLM text generation (OpenAI, Anthropic)
//! - Vector database semantic search (Qdrant)
//!
//! The Rust type system and compiler provide strong guardrails that prevent
//! common bugs and ensure code correctness at compile time.

mod api;
mod config;
mod models;
mod services;

use axum::{
    http::{header, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::SETTINGS;

/// Application entry point.
#[tokio::main]
async fn main() {
    // Initialize tracing/logging
    init_tracing();

    tracing::info!(
        environment = %SETTINGS.environment,
        "Starting backend server"
    );

    // Build application
    let app = create_app();

    // Bind address
    let addr = SocketAddr::from(([0, 0, 0, 0], SETTINGS.server.port));

    tracing::info!(
        address = %addr,
        cors_origins = ?SETTINGS.cors.origins,
        "Server listening"
    );

    // Start server
    if let Err(e) = run_server(addr, app).await {
        tracing::error!(error = %e, "Server failed");
        std::process::exit(1);
    }
}

/// Run the server with the given address and application.
async fn run_server(addr: SocketAddr, app: Router) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

/// Create the application router with all middleware.
fn create_app() -> Router {
    // Build CORS layer
    let cors = build_cors_layer();

    // Create API router
    let api_routes = api::create_router();

    // Build main router
    Router::new()
        // Health check at root
        .route("/", get(health_check))
        // API routes under /api prefix
        .nest("/api", api_routes)
        // Add middleware layers (order matters - applied bottom to top)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

/// Build CORS layer from configuration.
fn build_cors_layer() -> CorsLayer {
    let origins: Vec<_> = SETTINGS
        .cors
        .origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    let cors = if origins.is_empty() {
        // Fallback to any origin in development
        if SETTINGS.environment == config::Environment::Development {
            tracing::warn!("No CORS origins configured, allowing any origin (development mode)");
            CorsLayer::new().allow_origin(Any)
        } else {
            tracing::warn!("No CORS origins configured in production mode");
            CorsLayer::new()
        }
    } else {
        CorsLayer::new().allow_origin(origins)
    };

    cors.allow_methods([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
        Method::PATCH,
    ])
    .allow_headers([
        header::CONTENT_TYPE,
        header::AUTHORIZATION,
        header::ACCEPT,
        header::ORIGIN,
    ])
    .allow_credentials(true)
    .max_age(std::time::Duration::from_secs(600))
}

/// Health check endpoint.
async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "online",
            "environment": SETTINGS.environment.to_string(),
            "version": env!("CARGO_PKG_VERSION")
        })),
    )
}

/// Initialize tracing subscriber for logging.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "backend=debug,tower_http=debug,axum=debug".into());

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true),
        )
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let app = create_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
