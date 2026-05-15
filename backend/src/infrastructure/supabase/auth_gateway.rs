//! Supabase implementation of auth gateway contracts.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
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
            let body = response.text().await.map_err(|e| {
                AuthDomainError::ExternalService(format!(
                    "Supabase auth failed (status {status}) and body could not be read: {e}"
                ))
            })?;
            return Err(AuthDomainError::Unauthorized(format!(
                "Invalid token (status: {status}): {body}"
            )));
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
            let body = response.text().await.map_err(|e| {
                AuthDomainError::ExternalService(format!(
                    "Provider exchange failed (status {status}) and body could not be read: {e}"
                ))
            })?;
            return Err(AuthDomainError::BadRequest(format!(
                "Failed to authenticate with {provider} (status {status}): {body}"
            )));
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
