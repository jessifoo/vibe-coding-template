//! Supabase-specific transport models.

use serde::Deserialize;

use crate::domain::auth::AuthenticatedUser;

/// Supabase user payload returned by `GET /auth/v1/user`.
#[derive(Debug, Clone, Deserialize)]
pub struct SupabaseUserRecord {
    /// Supabase user id.
    pub id: String,
    /// Email if available.
    pub email: Option<String>,
    /// Nested user metadata.
    #[serde(default)]
    pub user_metadata: SupabaseUserMetadata,
}

/// Nested Supabase metadata.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SupabaseUserMetadata {
    /// Display full name.
    pub full_name: Option<String>,
    /// Avatar URL.
    pub avatar_url: Option<String>,
}

impl From<SupabaseUserRecord> for AuthenticatedUser {
    fn from(record: SupabaseUserRecord) -> Self {
        Self {
            id: record.id,
            email: record.email,
            full_name: record.user_metadata.full_name,
            avatar_url: record.user_metadata.avatar_url,
        }
    }
}
