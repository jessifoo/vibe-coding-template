//! Supabase database service — generic CRUD via `PostgREST`.

use std::collections::HashMap;
use std::fmt::Write;

use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};

use crate::config::SETTINGS;
use crate::models::AppError;
use crate::services::common::build_http_client;

/// Generic CRUD client for Supabase `PostgreSQL` tables.
#[derive(Clone)]
pub struct SupabaseDatabaseService {
    client: Client,
    base_url: String,
    service_key: String,
}

impl SupabaseDatabaseService {
    /// Create a new instance from global settings.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Configuration`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: build_http_client()?,
            base_url: format!("{}/rest/v1", SETTINGS.supabase.url),
            service_key: SETTINGS.supabase.service_key.clone(),
        })
    }

    // -- helpers ------------------------------------------------------------

    fn auth_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("apikey", &self.service_key)
            .bearer_auth(&self.service_key)
    }

    // -- CRUD ---------------------------------------------------------------

    /// List records, optionally filtered by equality conditions.
    pub async fn list<T: DeserializeOwned>(
        &self,
        table: &str,
        filters: Option<&HashMap<String, String>>,
    ) -> Result<Vec<T>, AppError> {
        let mut url = format!("{}/{table}?select=*", self.base_url);
        if let Some(f) = filters {
            for (k, v) in f {
                let _ = write!(url, "&{k}=eq.{v}");
            }
        }

        let resp = self
            .auth_headers(self.client.get(&url))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("DB query failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "DB query failed: {body}"
            )));
        }

        resp.json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Parse error: {e}")))
    }

    /// Get a single record by primary key.
    pub async fn get<T: DeserializeOwned>(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<T>, AppError> {
        let url = format!("{}/{table}?id=eq.{id}&select=*", self.base_url);

        let resp = self
            .auth_headers(self.client.get(&url))
            .header("Accept", "application/vnd.pgrst.object+json")
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("DB query failed: {e}")))?;

        if resp.status() == reqwest::StatusCode::NOT_ACCEPTABLE {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "DB query failed: {body}"
            )));
        }

        resp.json()
            .await
            .map(Some)
            .map_err(|e| AppError::ExternalService(format!("Parse error: {e}")))
    }

    /// Insert a new record, returning the created row.
    pub async fn create<T: DeserializeOwned, D: Serialize>(
        &self,
        table: &str,
        data: &D,
    ) -> Result<T, AppError> {
        let url = format!("{}/{table}", self.base_url);

        let resp = self
            .auth_headers(self.client.post(&url))
            .header("Prefer", "return=representation")
            .json(data)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("DB insert failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "DB insert failed: {body}"
            )));
        }

        let mut rows: Vec<T> = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Parse error: {e}")))?;

        rows.pop()
            .ok_or_else(|| AppError::ExternalService("Insert returned no data".into()))
    }

    /// Update a record by primary key, returning the updated row.
    pub async fn update<T: DeserializeOwned, D: Serialize>(
        &self,
        table: &str,
        id: &str,
        data: &D,
    ) -> Result<T, AppError> {
        let url = format!("{}/{table}?id=eq.{id}", self.base_url);

        let resp = self
            .auth_headers(self.client.patch(&url))
            .header("Prefer", "return=representation")
            .json(data)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("DB update failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "DB update failed: {body}"
            )));
        }

        let mut rows: Vec<T> = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Parse error: {e}")))?;

        rows.pop()
            .ok_or_else(|| AppError::NotFound(format!("Record {id} not found")))
    }

    /// Delete a record by primary key.
    pub async fn delete(&self, table: &str, id: &str) -> Result<bool, AppError> {
        let url = format!("{}/{table}?id=eq.{id}", self.base_url);

        let resp = self
            .auth_headers(self.client.delete(&url))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("DB delete failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "DB delete failed: {body}"
            )));
        }

        Ok(true)
    }
}
