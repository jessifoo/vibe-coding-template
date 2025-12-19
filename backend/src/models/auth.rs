//! Authentication models.
//!
//! Types for user authentication and authorization.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// User profile information returned from authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// Unique user identifier
    pub id: String,

    /// User email address
    pub email: String,

    /// User's full name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,

    /// URL to user's avatar image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

/// OAuth token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Access token
    pub access_token: String,

    /// Token type (usually "bearer")
    pub token_type: String,
}

impl TokenResponse {
    /// Create a new bearer token response.
    #[must_use]
    pub fn bearer(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            token_type: "bearer".to_string(),
        }
    }
}

/// Request to exchange a provider token for a Supabase token.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ProviderTokenRequest {
    /// OAuth provider (google, linkedin)
    #[validate(custom(function = "validate_provider"))]
    pub provider: String,

    /// Provider access token
    #[validate(length(min = 1, message = "Token cannot be empty"))]
    pub token: String,
}

/// Supported OAuth providers.
const SUPPORTED_PROVIDERS: &[&str] = &["google", "linkedin"];

/// Validate OAuth provider.
fn validate_provider(provider: &str) -> Result<(), validator::ValidationError> {
    if SUPPORTED_PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        let mut err = validator::ValidationError::new("invalid_provider");
        err.message = Some(
            format!(
                "Unsupported provider: {provider}. Supported: {}",
                SUPPORTED_PROVIDERS.join(", ")
            )
            .into(),
        );
        Err(err)
    }
}

/// Supabase user data from JWT.
#[derive(Debug, Clone, Deserialize)]
pub struct SupabaseUser {
    /// User ID
    pub id: String,

    /// User email
    pub email: Option<String>,

    /// User metadata
    #[serde(default)]
    pub user_metadata: UserMetadata,
}

/// User metadata from Supabase.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserMetadata {
    /// Full name
    pub full_name: Option<String>,

    /// Avatar URL
    pub avatar_url: Option<String>,
}

impl From<SupabaseUser> for UserProfile {
    fn from(user: SupabaseUser) -> Self {
        Self {
            id: user.id,
            email: user.email.unwrap_or_default(),
            full_name: user.user_metadata.full_name,
            avatar_url: user.user_metadata.avatar_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_response() {
        let response = TokenResponse::bearer("test_token");
        assert_eq!(response.access_token, "test_token");
        assert_eq!(response.token_type, "bearer");
    }

    #[test]
    fn test_validate_provider() {
        assert!(validate_provider("google").is_ok());
        assert!(validate_provider("linkedin").is_ok());
        assert!(validate_provider("invalid").is_err());
    }
}
