//! Authentication and user-profile types.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Authenticated user profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

/// OAuth token pair returned after authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
}

impl TokenResponse {
    /// Convenience constructor for bearer tokens.
    pub fn bearer(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            token_type: "bearer".to_string(),
        }
    }
}

/// Request body for provider-token exchange.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ProviderTokenRequest {
    #[validate(custom(function = "validate_provider"))]
    pub provider: String,

    #[validate(length(min = 1, message = "Token cannot be empty"))]
    pub token: String,
}

// ---------------------------------------------------------------------------
// Provider validation
// ---------------------------------------------------------------------------

const SUPPORTED_PROVIDERS: &[&str] = &["google", "linkedin"];

fn validate_provider(provider: &str) -> Result<(), validator::ValidationError> {
    if SUPPORTED_PROVIDERS.contains(&provider) {
        return Ok(());
    }
    let mut err = validator::ValidationError::new("invalid_provider");
    err.message = Some(
        format!(
            "Unsupported provider: {provider}. Supported: {}",
            SUPPORTED_PROVIDERS.join(", "),
        )
        .into(),
    );
    Err(err)
}

// ---------------------------------------------------------------------------
// Supabase JWT types (internal)
// ---------------------------------------------------------------------------

/// Raw user record from Supabase.
#[derive(Debug, Clone, Deserialize)]
pub struct SupabaseUser {
    pub id: String,
    pub email: Option<String>,
    #[serde(default)]
    pub user_metadata: UserMetadata,
}

/// Metadata embedded in the Supabase user record.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserMetadata {
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<SupabaseUser> for UserProfile {
    fn from(u: SupabaseUser) -> Self {
        Self {
            id: u.id,
            email: u.email.unwrap_or_default(),
            full_name: u.user_metadata.full_name,
            avatar_url: u.user_metadata.avatar_url,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_token_response() {
        let r = TokenResponse::bearer("tok");
        assert_eq!(r.access_token, "tok");
        assert_eq!(r.token_type, "bearer");
    }

    #[test]
    fn token_response_serialises_correctly() {
        let json = serde_json::to_string(&TokenResponse::bearer("x")).unwrap();
        assert!(json.contains("\"access_token\":\"x\""));
        assert!(json.contains("\"token_type\":\"bearer\""));
    }

    // -- provider validation ------------------------------------------------

    #[test]
    fn valid_providers_accepted() {
        assert!(validate_provider("google").is_ok());
        assert!(validate_provider("linkedin").is_ok());
    }

    #[test]
    fn invalid_provider_rejected() {
        let err = validate_provider("github").unwrap_err();
        assert_eq!(err.code, "invalid_provider");
    }

    #[test]
    fn empty_provider_rejected() {
        assert!(validate_provider("").is_err());
    }

    // -- ProviderTokenRequest validation ------------------------------------

    #[test]
    fn valid_request_passes() {
        let r = ProviderTokenRequest {
            provider: "google".into(),
            token: "t".into(),
        };
        assert!(r.validate().is_ok());
    }

    #[test]
    fn empty_token_rejected() {
        let r = ProviderTokenRequest {
            provider: "google".into(),
            token: String::new(),
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn bad_provider_rejected() {
        let r = ProviderTokenRequest {
            provider: "twitter".into(),
            token: "t".into(),
        };
        assert!(r.validate().is_err());
    }

    // -- SupabaseUser -> UserProfile ----------------------------------------

    #[test]
    fn supabase_user_converts_with_all_fields() {
        let u = SupabaseUser {
            id: "1".into(),
            email: Some("a@b.com".into()),
            user_metadata: UserMetadata {
                full_name: Some("Alice".into()),
                avatar_url: Some("https://img".into()),
            },
        };
        let p: UserProfile = u.into();
        assert_eq!(p.id, "1");
        assert_eq!(p.email, "a@b.com");
        assert_eq!(p.full_name.as_deref(), Some("Alice"));
        assert_eq!(p.avatar_url.as_deref(), Some("https://img"));
    }

    #[test]
    fn supabase_user_converts_with_missing_fields() {
        let u = SupabaseUser {
            id: "2".into(),
            email: None,
            user_metadata: UserMetadata::default(),
        };
        let p: UserProfile = u.into();
        assert_eq!(p.email, "");
        assert!(p.full_name.is_none());
    }

    // -- serialisation ------------------------------------------------------

    #[test]
    fn user_profile_omits_null_optionals() {
        let p = UserProfile {
            id: "1".into(),
            email: "a@b.com".into(),
            full_name: None,
            avatar_url: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("full_name"));
        assert!(!json.contains("avatar_url"));
    }

    #[test]
    fn user_profile_includes_present_optionals() {
        let p = UserProfile {
            id: "1".into(),
            email: "a@b.com".into(),
            full_name: Some("A".into()),
            avatar_url: Some("url".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("full_name"));
        assert!(json.contains("avatar_url"));
    }

    #[test]
    fn provider_token_request_deserialises() {
        let r: ProviderTokenRequest =
            serde_json::from_str(r#"{"provider":"google","token":"t"}"#).unwrap();
        assert_eq!(r.provider, "google");
    }
}
