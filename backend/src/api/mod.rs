//! API route definitions.
//!
//! Each sub-module owns a single domain and exposes a [`Router`](axum::Router).

pub mod auth;
pub mod llm;
pub mod vectordb;

use axum::Router;

/// Assemble the top-level `/api` router from all domain routers.
pub fn create_router() -> Router {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/llm", llm::router())
        .nest("/vectordb", vectordb::router())
}
