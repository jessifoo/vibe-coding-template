//! Backend library crate.
//!
//! Exposes modules for testing and reuse.

pub mod api;
pub mod app;
pub use app::run;
pub mod config;
pub mod http_auth;
pub mod models;
pub mod services;
