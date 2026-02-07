//! Backend library crate.
//!
//! Exposes modules for testing and reuse. Provides the application
//! builder so integration tests can construct the full router.

pub mod api;
pub mod config;
pub mod models;
pub mod services;

use axum::{
    http::{header, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::config::SETTINGS;

/// Create the application router with all middleware.
///
/// This is the main entry point for building the Axum application.
/// It wires up health checks, API routes, CORS, tracing, and request IDs.
///
/// # Returns
///
/// A fully configured `Router` ready to serve requests.
pub fn create_app() -> Router {
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
///
/// Returns a JSON response with server status, environment, and version.
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
