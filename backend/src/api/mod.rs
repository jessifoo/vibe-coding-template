//! API routes module.
//!
//! Contains all HTTP endpoint handlers organized by domain.

pub mod auth;
pub mod llm;
pub mod state;
pub mod vectordb;

use axum::Router;

use crate::api::state::AppState;

/// Create the main API router with all sub-routes.
pub fn create_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/llm", llm::router())
        .nest("/vectordb", vectordb::router())
}
