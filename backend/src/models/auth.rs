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
    use validator::Validate;

    #[test]
    fn test_token_response_bearer() {
        let response = TokenResponse::bearer("test_token");
        assert_eq!(response.access_token, "test_token");
        assert_eq!(response.token_type, "bearer");
    }

    #[test]
    fn test_token_response_bearer_from_string() {
        let response = TokenResponse::bearer(String::from("my_token"));
        assert_eq!(response.access_token, "my_token");
        assert_eq!(response.token_type, "bearer");
    }

    #[test]
    fn test_token_response_serialization() {
        let response = TokenResponse::bearer("abc123");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"access_token\":\"abc123\""));
        assert!(json.contains("\"token_type\":\"bearer\""));
    }

    #[test]
    fn test_validate_provider_google() {
        assert!(validate_provider("google").is_ok());
    }

    #[test]
    fn test_validate_provider_linkedin() {
        assert!(validate_provider("linkedin").is_ok());
    }

    #[test]
    fn test_validate_provider_invalid() {
        let result = validate_provider("github");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "invalid_provider");
    }

    #[test]
    fn test_validate_provider_empty() {
        assert!(validate_provider("").is_err());
    }

    #[test]
    fn test_provider_token_request_valid() {
        let request = ProviderTokenRequest {
            provider: "google".to_string(),
            token: "some_token".to_string(),
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_provider_token_request_empty_token() {
        let request = ProviderTokenRequest {
            provider: "google".to_string(),
            token: String::new(),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_provider_token_request_invalid_provider() {
        let request = ProviderTokenRequest {
            provider: "twitter".to_string(),
            token: "some_token".to_string(),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_supabase_user_to_user_profile() {
        let user = SupabaseUser {
            id: "user-123".to_string(),
            email: Some("test@example.com".to_string()),
            user_metadata: UserMetadata {
                full_name: Some("Test User".to_string()),
                avatar_url: Some("https://example.com/avatar.png".to_string()),
            },
        };
        let profile: UserProfile = user.into();
        assert_eq!(profile.id, "user-123");
        assert_eq!(profile.email, "test@example.com");
        assert_eq!(profile.full_name, Some("Test User".to_string()));
        assert_eq!(
            profile.avatar_url,
            Some("https://example.com/avatar.png".to_string())
        );
    }

    #[test]
    fn test_supabase_user_to_user_profile_no_email() {
        let user = SupabaseUser {
            id: "user-456".to_string(),
            email: None,
            user_metadata: UserMetadata::default(),
        };
        let profile: UserProfile = user.into();
        assert_eq!(profile.id, "user-456");
        assert_eq!(profile.email, ""); // unwrap_or_default
        assert!(profile.full_name.is_none());
        assert!(profile.avatar_url.is_none());
    }

    #[test]
    fn test_user_metadata_default() {
        let meta = UserMetadata::default();
        assert!(meta.full_name.is_none());
        assert!(meta.avatar_url.is_none());
    }

    #[test]
    fn test_user_profile_serialization_omits_nulls() {
        let profile = UserProfile {
            id: "1".to_string(),
            email: "test@test.com".to_string(),
            full_name: None,
            avatar_url: None,
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("full_name"));
        assert!(!json.contains("avatar_url"));
    }

    #[test]
    fn test_user_profile_serialization_includes_values() {
        let profile = UserProfile {
            id: "1".to_string(),
            email: "test@test.com".to_string(),
            full_name: Some("John".to_string()),
            avatar_url: Some("https://example.com/img.png".to_string()),
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"full_name\":\"John\""));
        assert!(json.contains("\"avatar_url\":\"https://example.com/img.png\""));
    }

    #[test]
    fn test_provider_token_request_deserialization() {
        let json = r#"{"provider":"google","token":"abc123"}"#;
        let req: ProviderTokenRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.provider, "google");
        assert_eq!(req.token, "abc123");
    }
}
