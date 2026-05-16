//! Shared HTTP application state.
//!
//! Long-lived, cloneable handles for Axum [`axum::extract::State`]. Construct
//! via [`AppState::try_new`]; do not create ad hoc in request handlers.

use std::sync::Arc;

use crate::application::auth::AuthUseCase;
use crate::infrastructure::supabase::auth_gateway::SupabaseAuthGateway;
use crate::models::AppRunError;
use crate::services::vectordb::qdrant::QdrantService;

/// Application state: shared service handles.
#[derive(Clone)]
pub struct AppState {
    /// Qdrant client; one per process, injected into handlers.
    pub qdrant: Arc<QdrantService>,

    /// Shared HTTP client for making external requests.
    pub reqwest_client: Arc<reqwest::Client>,

    /// Shared auth use-case (single auth entrypoint for protected APIs).
    pub auth_use_case: Arc<AuthUseCase>,
}

impl AppState {
    /// Build state by connecting to Qdrant (or the configured client).
    ///
    /// # Errors
    ///
    /// Returns [`AppRunError::QdrantInit`] if Qdrant initialization fails and
    /// [`AppRunError::HttpClientInit`] if HTTP client initialization fails.
    pub async fn try_new() -> Result<Self, AppRunError> {
        let qdrant = QdrantService::new()
            .await
            .map_err(AppRunError::QdrantInit)?;

        let reqwest_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| {
                AppRunError::HttpClientInit(crate::models::AppError::Configuration(format!(
                    "Failed to create HTTP client: {e}"
                )))
            })?;

        let reqwest_client = Arc::new(reqwest_client);
        let auth_gateway = SupabaseAuthGateway::new(Arc::clone(&reqwest_client));
        let auth_use_case = Arc::new(AuthUseCase::new(Arc::new(auth_gateway)));

        Ok(Self {
            qdrant: Arc::new(qdrant),
            reqwest_client,
            auth_use_case,
        })
    }
}
