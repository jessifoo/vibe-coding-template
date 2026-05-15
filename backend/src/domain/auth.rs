//! Domain contracts and entities for authentication.

use async_trait::async_trait;
use thiserror::Error;

/// Authenticated user entity used by application/business logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    /// Unique user identifier.
    pub id: String,
    /// User email if available.
    pub email: Option<String>,
    /// User full name if available.
    pub full_name: Option<String>,
    /// User avatar URL if available.
    pub avatar_url: Option<String>,
}

/// Domain-level auth errors (transport-agnostic).
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AuthDomainError {
    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    Unauthorized(String),
    /// Caller input is invalid.
    #[error("Invalid request: {0}")]
    BadRequest(String),
    /// External dependency failure.
    #[error("External service error: {0}")]
    ExternalService(String),
    /// Misconfiguration detected.
    #[error("Configuration error: {0}")]
    Configuration(String),
    /// Unexpected internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Gateway contract for auth providers (Supabase, etc).
#[async_trait]
pub trait AuthGateway: Send + Sync {
    /// Resolve user profile from a bearer token.
    ///
    /// # Errors
    ///
    /// Returns [`AuthDomainError`] when token validation or upstream calls fail.
    async fn get_user_from_bearer_token(
        &self,
        bearer_token: &str,
    ) -> Result<AuthenticatedUser, AuthDomainError>;

    /// Exchange a provider token (Google/LinkedIn/etc) for an app bearer token.
    ///
    /// # Errors
    ///
    /// Returns [`AuthDomainError`] when exchange fails.
    async fn exchange_provider_token(
        &self,
        provider: &str,
        provider_token: &str,
    ) -> Result<String, AuthDomainError>;
}
