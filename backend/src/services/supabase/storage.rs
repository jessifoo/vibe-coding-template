//! Supabase storage service.
//!
//! Handles file uploads and management via Supabase Storage.

use crate::config::SETTINGS;
use crate::http_error::read_failed_response_text;
use crate::models::AppError;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
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
            .timeout(std::time::Duration::from_secs(30))
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
            let (_status, body) = read_failed_response_text(response).await?;
            // Ignore "already exists" errors
            if !body.contains("already exists") {
                return Err(AppError::ExternalService(format!(
                    "Failed to create bucket: {body}"
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
        let safe_name = Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Invalid or empty filename (path segments not allowed)".to_string(),
                )
            })?;
        // Generate unique object key (no user-controlled path segments in the basename)
        let unique_filename = format!("{}-{}", Uuid::new_v4(), safe_name);
        let full_path = match path {
            Some(p) if p.contains("..") || p.starts_with('/') => {
                return Err(AppError::BadRequest(
                    "Storage path must be relative and cannot contain '..'".to_string(),
                ));
            }
            Some(p) => {
                // URL-encode each path segment individually
                let encoded_segments: Vec<String> = p
                    .split('/')
                    .map(|seg| urlencoding::encode(seg).into_owned())
                    .collect();
                let encoded_path = encoded_segments.join("/");
                format!("{}/{}", encoded_path, urlencoding::encode(&unique_filename))
            }
            None => urlencoding::encode(&unique_filename).into_owned(),
        };

        let url = format!(
            "{}/object/{}/{}",
            self.base_url,
            urlencoding::encode(&self.bucket_name),
            full_path
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
            let (status, body) = read_failed_response_text(response).await?;
            return Err(AppError::ExternalService(format!(
                "File upload failed (status {status}): {body}"
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
        // Validate path
        if path.is_empty() {
            return Err(AppError::BadRequest(
                "File path cannot be empty".to_string(),
            ));
        }
        if path.contains("..") || path.starts_with('/') {
            return Err(AppError::BadRequest(
                "File path must be relative and cannot contain '..'".to_string(),
            ));
        }

        // URL-encode each path segment
        let encoded_segments: Vec<String> = path
            .split('/')
            .map(|seg| urlencoding::encode(seg).into_owned())
            .collect();
        let encoded_path = encoded_segments.join("/");

        let url = format!(
            "{}/object/{}/{}",
            self.base_url,
            urlencoding::encode(&self.bucket_name),
            encoded_path
        );

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
            let (status, body) = read_failed_response_text(response).await?;
            return Err(AppError::ExternalService(format!(
                "Failed to list files (status {status}): {body}"
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
