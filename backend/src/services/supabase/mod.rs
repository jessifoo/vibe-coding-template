//! Supabase service integrations.
//!
//! Provides authentication, database, and storage services via Supabase.

pub mod auth;
pub mod database;
pub mod storage;

pub use auth::SupabaseAuthService;
pub use database::SupabaseDatabaseService;
pub use storage::SupabaseStorageService;
