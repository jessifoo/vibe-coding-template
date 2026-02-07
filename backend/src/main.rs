//! Full Stack Application Backend
//!
//! A Rust backend built with Axum, featuring:
//! - Supabase authentication and database integration
//! - LLM text generation (`OpenAI`, Anthropic)
//! - Vector database semantic search (Qdrant)
//!
//! The Rust type system and compiler provide strong guardrails that prevent
//! common bugs and ensure code correctness at compile time.

use backend::config::SETTINGS;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    let app = backend::create_app();

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
async fn run_server(addr: SocketAddr, app: axum::Router) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
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
