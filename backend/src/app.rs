//! HTTP application factory: router, state, and middleware.
//!
//! Centralizes Axum setup so the binary only calls [`run`], and integration
//! tests can call [`build_app`] the same way production does.

use std::net::SocketAddr;

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

use crate::api::state::AppState;
use crate::config::{Environment, SETTINGS};
use crate::models::AppRunError;

/// Build the full Axum [`Router`], including shared [`AppState`] and middleware.
///
/// # Errors
///
/// Returns [`AppRunError`] when a long-lived resource (e.g. Qdrant) cannot be
/// initialized.
///
/// The returned [`Router`] is a [`Router<()>`](Router): routes were built with
/// [`Router<AppState>`](AppState) and then completed with
/// [`Router::with_state`](Router::with_state), which is the pattern Axum
/// documents for `axum::serve`.
pub async fn build_app() -> Result<Router, AppRunError> {
    let app_state = AppState::try_new().await?;
    let cors = build_cors_layer();

    Ok(Router::new()
        .route("/", get(health_check))
        .nest("/api", crate::api::create_router())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(app_state))
}

/// Build CORS from [`SETTINGS`].
fn build_cors_layer() -> CorsLayer {
    let origins: Vec<_> = SETTINGS
        .cors
        .origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    let mut cors = if origins.is_empty() {
        if SETTINGS.environment == Environment::Development {
            tracing::warn!("No CORS origins configured, allowing any origin (development mode)");
            CorsLayer::new().allow_origin(Any)
        } else {
            tracing::warn!("No CORS origins configured in production; CORS will reject requests");
            CorsLayer::new()
        }
    } else {
        // Only enable credentials when using specific origins
        CorsLayer::new()
            .allow_origin(origins)
            .allow_credentials(true)
    };

    cors = cors
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
        .max_age(std::time::Duration::from_secs(600));

    cors
}

/// Health check at `/`.
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

/// Run the server: bind and serve (library entry for binary and tests).
pub async fn run() -> Result<(), AppRunError> {
    init_tracing();

    tracing::info!(
        environment = %SETTINGS.environment,
        "Starting backend server"
    );

    let app = build_app().await?;
    let bind_address = format!("{}:{}", SETTINGS.server.host, SETTINGS.server.port);
    let addr: SocketAddr = bind_address.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid bind address '{}': {}", bind_address, e),
        )
    })?;

    tracing::info!(
        address = %addr,
        cors_origins = ?SETTINGS.cors.origins,
        "Server listening"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "backend=debug,tower_http=debug,axum=debug".into());

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true),
        )
        .try_init();
}
