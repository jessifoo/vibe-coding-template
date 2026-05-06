//! Supabase authentication service.
//!
//! Handles JWT token verification and user authentication via Supabase.

use crate::config::SETTINGS;
use crate::http_auth::bearer_token_from_value;
use crate::http_error::read_failed_response_text;
use crate::models::{AppError, SupabaseUser, UserProfile};
use reqwest::Client;
use serde::Deserialize;

/// Service for handling Supabase authentication.
#[derive(Clone)]
pub struct SupabaseAuthService {
    client: Client,
    supabase_url: String,
    service_key: String,
}

impl SupabaseAuthService {
    /// Create a new Supabase auth service.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created or if configuration is invalid.
    pub fn new() -> Result<Self, AppError> {
        let settings = SETTINGS.as_ref()
            .map_err(|e| AppError::Configuration(format!("Failed to load settings: {e}")))?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            supabase_url: settings.supabase.url.clone(),
            service_key: settings.supabase.service_key.clone(),
        })
    }

    /// Get user data from a JWT token.
    ///
    /// # Arguments
    ///
    /// * `jwt_token` - The JWT token from the Authorization header
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or the user cannot be fetched.
    pub async fn get_user(&self, jwt_token: &str) -> Result<UserProfile, AppError> {
        let url = format!("{}/auth/v1/user", self.supabase_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {jwt_token}"))
            .header("apikey", &self.service_key)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Supabase request failed: {e}")))?;

        if !response.status().is_success() {
            let (status, body) = read_failed_response_text(response).await?;
            return Err(AppError::Unauthorized(format!(
                "Invalid token (status: {status}): {body}"
            )));
        }

        let user: SupabaseUser = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse user data: {e}")))?;

        Ok(user.into())
    }

    /// Exchange a provider token for a Supabase token.
    ///
    /// # Arguments
    ///
    /// * `provider` - OAuth provider (google, linkedin) - assumed to be validated by caller
    /// * `access_token` - Provider access token
    ///
    /// # Errors
    ///
    /// Returns an error if the exchange fails.
    pub async fn sign_in_with_provider_token(
        &self,
        provider: &str,
        access_token: &str,
    ) -> Result<String, AppError> {
        // Note: Provider validation is handled by the ProviderTokenRequest validator
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
                id_token: access_token,
            })
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Provider token exchange failed: {e}"))
            })?;

        if !response.status().is_success() {
            let (status, body) = read_failed_response_text(response).await?;
            return Err(AppError::BadRequest(format!(
                "Failed to authenticate with {provider} (status {status}): {body}"
            )));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to parse token response: {e}"))
        })?;

        Ok(token_response.access_token)
    }
}

/// Extract bearer token from Authorization header.
///
/// # Arguments
///
/// * `header` - The Authorization header value
///
/// # Returns
///
/// The token without the "Bearer " prefix, or an error if invalid.
pub fn extract_bearer_token(header: &str) -> Result<&str, AppError> {
    bearer_token_from_value(header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(
            extract_bearer_token("Bearer test_token").expect("bearer token"),
            "test_token"
        );
        assert_eq!(
            extract_bearer_token("bearer test_token").expect("bearer token"),
            "test_token"
        );
        assert!(extract_bearer_token("Basic test_token").is_err());
        assert!(extract_bearer_token("test_token").is_err());
    }
}
