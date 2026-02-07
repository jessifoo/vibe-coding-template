//! Supabase database service.
//!
//! Generic CRUD operations for Supabase PostgreSQL database via REST API.

use crate::config::SETTINGS;
use crate::models::AppError;
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;

/// Service for interacting with Supabase database.
#[derive(Clone)]
pub struct SupabaseDatabaseService {
    client: Client,
    base_url: String,
    service_key: String,
}

impl SupabaseDatabaseService {
    /// Create a new database service instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: format!("{}/rest/v1", SETTINGS.supabase.url),
            service_key: SETTINGS.supabase.service_key.clone(),
        })
    }

    /// List records from a table with optional filters.
    ///
    /// # Arguments
    ///
    /// * `table` - Table name
    /// * `filters` - Optional filters (key=value equality)
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn list<T: DeserializeOwned>(
        &self,
        table: &str,
        filters: Option<&HashMap<String, String>>,
    ) -> Result<Vec<T>, AppError> {
        let mut url = format!("{}/{table}?select=*", self.base_url);

        if let Some(filters) = filters {
            for (key, value) in filters {
                url.push_str(&format!("&{key}=eq.{value}"));
            }
        }

        let response = self
            .client
            .get(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Database query failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Database query failed: {error}"
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse response: {e}")))
    }

    /// Get a single record by ID.
    ///
    /// # Arguments
    ///
    /// * `table` - Table name
    /// * `id` - Record ID
    ///
    /// # Errors
    ///
    /// Returns an error if the record is not found or the query fails.
    pub async fn get<T: DeserializeOwned>(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<T>, AppError> {
        let url = format!("{}/{table}?id=eq.{id}&select=*", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .header("Accept", "application/vnd.pgrst.object+json")
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Database query failed: {e}")))?;

        if response.status() == reqwest::StatusCode::NOT_ACCEPTABLE {
            // No record found
            return Ok(None);
        }

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Database query failed: {error}"
            )));
        }

        response
            .json()
            .await
            .map(Some)
            .map_err(|e| AppError::ExternalService(format!("Failed to parse response: {e}")))
    }

    /// Create a new record.
    ///
    /// # Arguments
    ///
    /// * `table` - Table name
    /// * `data` - Record data to insert
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails.
    pub async fn create<T: DeserializeOwned, D: Serialize>(
        &self,
        table: &str,
        data: &D,
    ) -> Result<T, AppError> {
        let url = format!("{}/{table}", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation")
            .json(data)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Database insert failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Database insert failed: {error}"
            )));
        }

        // Response is an array, get first element
        let mut records: Vec<T> = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse response: {e}")))?;

        records
            .pop()
            .ok_or_else(|| AppError::ExternalService("Insert returned no data".to_string()))
    }

    /// Update a record by ID.
    ///
    /// # Arguments
    ///
    /// * `table` - Table name
    /// * `id` - Record ID
    /// * `data` - Fields to update
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub async fn update<T: DeserializeOwned, D: Serialize>(
        &self,
        table: &str,
        id: &str,
        data: &D,
    ) -> Result<T, AppError> {
        let url = format!("{}/{table}?id=eq.{id}", self.base_url);

        let response = self
            .client
            .patch(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation")
            .json(data)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Database update failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Database update failed: {error}"
            )));
        }

        let mut records: Vec<T> = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse response: {e}")))?;

        records
            .pop()
            .ok_or_else(|| AppError::NotFound(format!("Record with ID {id} not found")))
    }

    /// Delete a record by ID.
    ///
    /// # Arguments
    ///
    /// * `table` - Table name
    /// * `id` - Record ID
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    pub async fn delete(&self, table: &str, id: &str) -> Result<bool, AppError> {
        let url = format!("{}/{table}?id=eq.{id}", self.base_url);

        let response = self
            .client
            .delete(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Database delete failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Database delete failed: {error}"
            )));
        }

        Ok(true)
    }
}

impl Default for SupabaseDatabaseService {
    /// Create a default database service.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created.
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new().expect("Failed to create default SupabaseDatabaseService")
    }
}
