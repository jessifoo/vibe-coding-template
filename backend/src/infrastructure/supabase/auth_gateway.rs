//! Supabase implementation of auth gateway contracts.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::config::SETTINGS;
use crate::domain::auth::{AuthDomainError, AuthGateway, AuthenticatedUser};
use crate::infrastructure::supabase::models::SupabaseUserRecord;

/// Auth gateway backed by Supabase Auth REST endpoints.
#[derive(Clone)]
pub struct SupabaseAuthGateway {
    client: Arc<Client>,
    supabase_url: String,
    service_key: String,
}

impl SupabaseAuthGateway {
    /// Build a Supabase auth gateway from shared infrastructure dependencies.
    #[must_use]
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            supabase_url: SETTINGS.supabase.url.clone(),
            service_key: SETTINGS.supabase.service_key.clone(),
        }
    }
}

fn map_user_lookup_error(status: StatusCode) -> AuthDomainError {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return AuthDomainError::Unauthorized("Invalid bearer token".to_string());
    }

    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return AuthDomainError::ExternalService(format!(
            "Supabase auth service unavailable (status {status})"
        ));
    }

    if status.is_client_error() {
        return AuthDomainError::BadRequest(format!(
            "Supabase auth request rejected (status {status})"
        ));
    }

    AuthDomainError::ExternalService(format!("Supabase auth failed (status {status})"))
}

fn map_provider_exchange_error(status: StatusCode, provider: &str) -> AuthDomainError {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return AuthDomainError::ExternalService(format!(
            "Provider token exchange failed for {provider} (status {status})"
        ));
    }

    if status.is_client_error() {
        return AuthDomainError::BadRequest(format!(
            "Failed to authenticate with {provider} (status {status})"
        ));
    }

    AuthDomainError::ExternalService(format!(
        "Provider token exchange failed for {provider} (status {status})"
    ))
}

#[async_trait]
impl AuthGateway for SupabaseAuthGateway {
    async fn get_user_from_bearer_token(
        &self,
        bearer_token: &str,
    ) -> Result<AuthenticatedUser, AuthDomainError> {
        let url = format!("{}/auth/v1/user", self.supabase_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {bearer_token}"))
            .header("apikey", &self.service_key)
            .send()
            .await
            .map_err(|e| {
                AuthDomainError::ExternalService(format!("Supabase request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::warn!(status = %status, "Supabase bearer token lookup failed");
            return Err(map_user_lookup_error(status));
        }

        let user: SupabaseUserRecord = response.json().await.map_err(|e| {
            AuthDomainError::ExternalService(format!("Failed to parse Supabase user payload: {e}"))
        })?;

        Ok(user.into())
    }

    async fn exchange_provider_token(
        &self,
        provider: &str,
        provider_token: &str,
    ) -> Result<String, AuthDomainError> {
        let url = format!("{}/auth/v1/token?grant_type=id_token", self.supabase_url);

        #[derive(serde::Serialize)]
        struct TokenRequest<'a> {
            provider: &'a str,
            id_token: &'a str,
        }

        let response = self
            .client
            .post(&url)
            .header("apikey", &self.service_key)
            .header("Content-Type", "application/json")
            .json(&TokenRequest {
                provider,
                id_token: provider_token,
            })
            .send()
            .await
            .map_err(|e| {
                AuthDomainError::ExternalService(format!("Provider token exchange failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::warn!(
                provider = provider,
                status = %status,
                "Supabase provider token exchange failed"
            );
            return Err(map_provider_exchange_error(status, provider));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            AuthDomainError::ExternalService(format!(
                "Failed to parse Supabase provider token response: {e}"
            ))
        })?;
        Ok(token_response.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::{map_provider_exchange_error, map_user_lookup_error};
    use crate::domain::auth::AuthDomainError;
    use reqwest::StatusCode;

    #[test]
    fn user_lookup_maps_unauthorized_statuses() {
        assert!(matches!(
            map_user_lookup_error(StatusCode::UNAUTHORIZED),
            AuthDomainError::Unauthorized(_)
        ));
        assert!(matches!(
            map_user_lookup_error(StatusCode::FORBIDDEN),
            AuthDomainError::Unauthorized(_)
        ));
    }

    #[test]
    fn user_lookup_maps_upstream_outages_to_external_service() {
        assert!(matches!(
            map_user_lookup_error(StatusCode::TOO_MANY_REQUESTS),
            AuthDomainError::ExternalService(_)
        ));
        assert!(matches!(
            map_user_lookup_error(StatusCode::BAD_GATEWAY),
            AuthDomainError::ExternalService(_)
        ));
    }

    #[test]
    fn provider_exchange_maps_status_classes() {
        assert!(matches!(
            map_provider_exchange_error(StatusCode::BAD_REQUEST, "google"),
            AuthDomainError::BadRequest(_)
        ));
        assert!(matches!(
            map_provider_exchange_error(StatusCode::TOO_MANY_REQUESTS, "google"),
            AuthDomainError::ExternalService(_)
        ));
        assert!(matches!(
            map_provider_exchange_error(StatusCode::INTERNAL_SERVER_ERROR, "google"),
            AuthDomainError::ExternalService(_)
        ));
    }
}
