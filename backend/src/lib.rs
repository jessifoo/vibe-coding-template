//! Backend library crate.
//!
//! Exposes modules for testing and reuse.

pub mod api;
pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod http_auth;
pub mod http_error;
pub mod infrastructure;
pub mod models;
pub mod services;

pub use app::run;
