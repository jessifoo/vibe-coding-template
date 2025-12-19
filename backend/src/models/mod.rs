//! API request and response models.
//!
//! All models use serde for serialization and validator for validation.
//! The Rust type system ensures these models are always valid at compile time.

pub mod auth;
pub mod error;
pub mod llm;
pub mod vectordb;

pub use auth::*;
pub use error::*;
pub use llm::*;
pub use vectordb::*;
