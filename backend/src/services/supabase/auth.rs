//! Supabase authentication service.

use reqwest::Client;
use serde::Deserialize;

use crate::config::SETTINGS;
use crate::models::{AppError, SupabaseUser, UserProfile};
use crate::services::common::build_http_client;

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
}

/// Verifies JWTs and exchanges provider tokens via the Supabase Auth API.
#[derive(Clone)]
pub struct SupabaseAuthService {
    client: Client,
    supabase_url: String,
    service_key: String,
}

impl SupabaseAuthService {
    /// Create a new instance from global [`SETTINGS`](crate::config::SETTINGS).
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Configuration`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: build_http_client()?,
            supabase_url: SETTINGS.supabase.url.clone(),
            service_key: SETTINGS.supabase.service_key.clone(),
        })
    }

    /// Fetch the user profile for a given JWT.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Unauthorized`] for invalid tokens.
    pub async fn get_user(&self, jwt: &str) -> Result<UserProfile, AppError> {
        let url = format!("{}/auth/v1/user", self.supabase_url);

        let resp = self
            .client
            .get(&url)
            .bearer_auth(jwt)
            .header("apikey", &self.service_key)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Supabase request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Unauthorized(format!(
                "Invalid token ({status}): {body}"
            )));
        }

        let user: SupabaseUser = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("User data parse error: {e}")))?;

        Ok(user.into())
    }

    /// Exchange an OAuth provider token for a Supabase access token.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::BadRequest`] for unsupported providers or exchange failures.
    pub async fn sign_in_with_provider_token(
        &self,
        provider: &str,
        access_token: &str,
    ) -> Result<String, AppError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            provider: &'a str,
            id_token: &'a str,
        }

        if !["google", "linkedin"].contains(&provider) {
            return Err(AppError::BadRequest(format!(
                "Unsupported provider: {provider}"
            )));
        }

        let url = format!("{}/auth/v1/token?grant_type=id_token", self.supabase_url);

        let resp = self
            .client
            .post(&url)
            .header("apikey", &self.service_key)
            .json(&Body {
                provider,
                id_token: access_token,
            })
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Token exchange failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!(
                "Auth with {provider} failed: {body}"
            )));
        }

        let token: TokenResp = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Token parse error: {e}")))?;

        Ok(token.access_token)
    }
}
