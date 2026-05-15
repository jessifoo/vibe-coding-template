//! Authentication application use-cases.

use std::sync::Arc;

use crate::domain::auth::{AuthDomainError, AuthGateway, AuthenticatedUser};

/// Use-cases for auth workflows.
#[derive(Clone)]
pub struct AuthUseCase {
    gateway: Arc<dyn AuthGateway>,
}

impl AuthUseCase {
    /// Create auth use-cases with the configured gateway adapter.
    #[must_use]
    pub fn new(gateway: Arc<dyn AuthGateway>) -> Self {
        Self { gateway }
    }

    /// Authenticate a request bearer token and return domain user data.
    ///
    /// # Errors
    ///
    /// Returns [`AuthDomainError`] when token parsing or provider lookup fails.
    pub async fn authenticate_with_bearer_token(
        &self,
        bearer_token: &str,
    ) -> Result<AuthenticatedUser, AuthDomainError> {
        self.gateway.get_user_from_bearer_token(bearer_token).await
    }

    /// Exchange OAuth provider token for application bearer token.
    ///
    /// # Errors
    ///
    /// Returns [`AuthDomainError`] when exchange fails.
    pub async fn exchange_provider_token(
        &self,
        provider: &str,
        provider_token: &str,
    ) -> Result<String, AuthDomainError> {
        self.gateway
            .exchange_provider_token(provider, provider_token)
            .await
    }
}
