//! Backend library crate.
//!
//! Exposes modules for testing and reuse.

pub mod api;
pub mod app;
pub mod config;
pub mod http_auth;
pub mod http_error;
pub mod models;
pub mod services;

pub use app::run;
