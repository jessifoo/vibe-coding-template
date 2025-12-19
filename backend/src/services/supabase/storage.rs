//! Supabase storage service.
//!
//! Handles file uploads and management via Supabase Storage.

use crate::config::SETTINGS;
use crate::models::AppError;
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// Service for interacting with Supabase Storage.
#[derive(Clone)]
pub struct SupabaseStorageService {
    client: Client,
    base_url: String,
    service_key: String,
    bucket_name: String,
}

impl SupabaseStorageService {
    /// Create a new storage service instance.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - Name of the storage bucket
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(bucket_name: &str) -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: format!("{}/storage/v1", SETTINGS.supabase.url),
            service_key: SETTINGS.supabase.service_key.clone(),
            bucket_name: bucket_name.to_string(),
        })
    }

    /// Ensure the bucket exists, creating it if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket creation fails.
    pub async fn ensure_bucket_exists(&self) -> Result<(), AppError> {
        // Try to get the bucket
        let url = format!("{}/bucket/{}", self.base_url, self.bucket_name);

        let response = self
            .client
            .get(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Storage request failed: {e}")))?;

        if response.status().is_success() {
            return Ok(());
        }

        // Bucket doesn't exist, create it
        let create_url = format!("{}/bucket", self.base_url);

        #[derive(serde::Serialize)]
        struct CreateBucketRequest<'a> {
            name: &'a str,
            public: bool,
        }

        let response = self
            .client
            .post(&create_url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .json(&CreateBucketRequest {
                name: &self.bucket_name,
                public: false,
            })
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to create bucket: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            // Ignore "already exists" errors
            if !error.contains("already exists") {
                return Err(AppError::ExternalService(format!(
                    "Failed to create bucket: {error}"
                )));
            }
        }

        Ok(())
    }

    /// Upload a file to storage.
    ///
    /// # Arguments
    ///
    /// * `filename` - Original filename
    /// * `content` - File content bytes
    /// * `content_type` - MIME type
    /// * `path` - Optional path prefix within the bucket
    ///
    /// # Returns
    ///
    /// The public URL of the uploaded file.
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub async fn upload_file(
        &self,
        filename: &str,
        content: Vec<u8>,
        content_type: &str,
        path: Option<&str>,
    ) -> Result<String, AppError> {
        // Generate unique filename
        let unique_filename = format!("{}-{filename}", Uuid::new_v4());
        let full_path = match path {
            Some(p) => format!("{p}/{unique_filename}"),
            None => unique_filename,
        };

        let url = format!(
            "{}/object/{}/{}",
            self.base_url, self.bucket_name, full_path
        );

        let response = self
            .client
            .post(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .header("Content-Type", content_type)
            .body(content)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("File upload failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "File upload failed: {error}"
            )));
        }

        // Return public URL
        Ok(self.get_public_url(&full_path))
    }

    /// Get the public URL for a file.
    ///
    /// # Arguments
    ///
    /// * `path` - File path within the bucket
    #[must_use]
    pub fn get_public_url(&self, path: &str) -> String {
        format!(
            "{}/object/public/{}/{}",
            self.base_url, self.bucket_name, path
        )
    }

    /// Delete a file from storage.
    ///
    /// # Arguments
    ///
    /// * `path` - File path within the bucket
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub async fn delete_file(&self, path: &str) -> Result<bool, AppError> {
        let url = format!("{}/object/{}/{}", self.base_url, self.bucket_name, path);

        let response = self
            .client
            .delete(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("File deletion failed: {e}")))?;

        Ok(response.status().is_success())
    }

    /// List files in a directory.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path (optional)
    ///
    /// # Errors
    ///
    /// Returns an error if the listing fails.
    pub async fn list_files(&self, path: Option<&str>) -> Result<Vec<FileInfo>, AppError> {
        let url = format!("{}/object/list/{}", self.base_url, self.bucket_name);

        #[derive(serde::Serialize)]
        struct ListRequest<'a> {
            prefix: &'a str,
        }

        let response = self
            .client
            .post(&url)
            .header("apikey", &self.service_key)
            .header("Authorization", format!("Bearer {}", self.service_key))
            .json(&ListRequest {
                prefix: path.unwrap_or(""),
            })
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list files: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Failed to list files: {error}"
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse file list: {e}")))
    }
}

/// File information from storage listing.
#[derive(Debug, Clone, Deserialize)]
pub struct FileInfo {
    /// File name
    pub name: String,

    /// File ID
    pub id: Option<String>,

    /// Last updated timestamp
    pub updated_at: Option<String>,

    /// Creation timestamp
    pub created_at: Option<String>,

    /// File size in bytes
    #[serde(default)]
    pub metadata: FileMetadata,
}

/// File metadata.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes
    pub size: Option<u64>,

    /// MIME type
    pub mimetype: Option<String>,
}
