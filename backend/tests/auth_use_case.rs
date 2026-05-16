#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Integration tests for the auth application use-case.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::application::auth::AuthUseCase;
use backend::domain::auth::{AuthDomainError, AuthGateway, AuthenticatedUser};

#[derive(Clone)]
struct MockAuthGateway {
    user_result: Result<AuthenticatedUser, AuthDomainError>,
    token_result: Result<String, AuthDomainError>,
    seen_bearer_tokens: Arc<Mutex<Vec<String>>>,
    seen_provider_tokens: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl AuthGateway for MockAuthGateway {
    async fn get_user_from_bearer_token(
        &self,
        bearer_token: &str,
    ) -> Result<AuthenticatedUser, AuthDomainError> {
        self.seen_bearer_tokens
            .lock()
            .expect("lock seen_bearer_tokens")
            .push(bearer_token.to_string());
        self.user_result.clone()
    }

    async fn exchange_provider_token(
        &self,
        provider: &str,
        provider_token: &str,
    ) -> Result<String, AuthDomainError> {
        self.seen_provider_tokens
            .lock()
            .expect("lock seen_provider_tokens")
            .push((provider.to_string(), provider_token.to_string()));
        self.token_result.clone()
    }
}

fn sample_user() -> AuthenticatedUser {
    AuthenticatedUser {
        id: "user-123".to_string(),
        email: Some("user@example.com".to_string()),
        full_name: Some("Example User".to_string()),
        avatar_url: Some("https://example.com/avatar.png".to_string()),
    }
}

#[tokio::test]
async fn authenticate_with_bearer_token_returns_gateway_user() {
    let seen_bearer_tokens = Arc::new(Mutex::new(Vec::new()));
    let gateway = MockAuthGateway {
        user_result: Ok(sample_user()),
        token_result: Ok("unused".to_string()),
        seen_bearer_tokens: Arc::clone(&seen_bearer_tokens),
        seen_provider_tokens: Arc::new(Mutex::new(Vec::new())),
    };
    let use_case = AuthUseCase::new(Arc::new(gateway));

    let user = use_case
        .authenticate_with_bearer_token("token-abc")
        .await
        .expect("authenticate_with_bearer_token");

    assert_eq!(user.id, "user-123");
    assert_eq!(
        seen_bearer_tokens
            .lock()
            .expect("lock seen_bearer_tokens")
            .as_slice(),
        &["token-abc".to_string()]
    );
}

#[tokio::test]
async fn exchange_provider_token_delegates_to_gateway() {
    let seen_provider_tokens = Arc::new(Mutex::new(Vec::new()));
    let gateway = MockAuthGateway {
        user_result: Ok(sample_user()),
        token_result: Ok("supabase-access-token".to_string()),
        seen_bearer_tokens: Arc::new(Mutex::new(Vec::new())),
        seen_provider_tokens: Arc::clone(&seen_provider_tokens),
    };
    let use_case = AuthUseCase::new(Arc::new(gateway));

    let token = use_case
        .exchange_provider_token("google", "provider-token")
        .await
        .expect("exchange_provider_token");

    assert_eq!(token, "supabase-access-token");
    assert_eq!(
        seen_provider_tokens
            .lock()
            .expect("lock seen_provider_tokens")
            .as_slice(),
        &[("google".to_string(), "provider-token".to_string())]
    );
}

#[tokio::test]
async fn authenticate_with_bearer_token_propagates_gateway_error() {
    let gateway = MockAuthGateway {
        user_result: Err(AuthDomainError::Unauthorized(
            "Invalid bearer token".to_string(),
        )),
        token_result: Ok("unused".to_string()),
        seen_bearer_tokens: Arc::new(Mutex::new(Vec::new())),
        seen_provider_tokens: Arc::new(Mutex::new(Vec::new())),
    };
    let use_case = AuthUseCase::new(Arc::new(gateway));

    let error = use_case
        .authenticate_with_bearer_token("bad-token")
        .await
        .expect_err("authenticate_with_bearer_token should fail");

    match error {
        AuthDomainError::Unauthorized(message) => {
            assert!(message.contains("Invalid bearer token"));
        }
        _ => panic!("expected AuthDomainError::Unauthorized"),
    }
}
