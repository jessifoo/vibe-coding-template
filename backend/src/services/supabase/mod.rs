//! Supabase service integrations (auth, database, storage).

pub mod auth;
pub mod database;
pub mod storage;

pub use auth::SupabaseAuthService;
pub use database::SupabaseDatabaseService;
pub use storage::SupabaseStorageService;
