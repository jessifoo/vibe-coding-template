//! Backend library crate.
//!
//! Exposes all modules for integration testing and binary reuse.
//! The [`create_app`] function is the main entry point for assembling the Axum router.

// Allow unwrap/expect in test code — standard Rust test practice.
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::manual_string_new)
)]

pub mod api;
pub mod config;
pub mod models;
pub mod services;
pub mod utils;

use axum::{
    Json, Router,
    http::{Method, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::config::SETTINGS;

/// Build the fully-configured application [`Router`].
///
/// Wires up the health-check endpoint, all API sub-routes,
/// CORS, request tracing, and request-ID propagation.
pub fn create_app() -> Router {
    let cors = build_cors_layer();
    let api_routes = api::create_router();

    Router::new()
        .route("/", get(health_check))
        .nest("/api", api_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

/// Health-check endpoint (`GET /`).
///
/// Returns server status, environment, and crate version.
async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "online",
            "environment": SETTINGS.environment.to_string(),
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
}

/// Construct the CORS middleware from [`SETTINGS`].
fn build_cors_layer() -> CorsLayer {
    let parsed_origins: Vec<_> = SETTINGS
        .cors
        .origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    let origin_layer = if parsed_origins.is_empty() {
        if SETTINGS.environment == config::Environment::Development {
            tracing::warn!("No CORS origins configured — allowing any origin (development mode)");
            CorsLayer::new().allow_origin(Any)
        } else {
            tracing::warn!("No CORS origins configured in production mode");
            CorsLayer::new()
        }
    } else {
        CorsLayer::new().allow_origin(AllowOrigin::list(parsed_origins))
    };

    origin_layer
        .allow_methods([
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
