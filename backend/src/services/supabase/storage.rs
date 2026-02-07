//! Supabase storage service — file upload, download, listing.

use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

use crate::config::SETTINGS;
use crate::models::AppError;

#[derive(serde::Serialize)]
struct CreateBucket<'a> {
    name: &'a str,
    public: bool,
}

/// Client for the Supabase Storage API.
#[derive(Clone)]
pub struct SupabaseStorageService {
    client: Client,
    base_url: String,
    service_key: String,
    bucket_name: String,
}

impl SupabaseStorageService {
    /// Create a new storage service for `bucket_name`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Configuration`] if the HTTP client cannot be built.
    pub fn new(bucket_name: &str) -> Result<Self, AppError> {
        let client = Client::builder()
            .build()
            .map_err(|e| AppError::Configuration(format!("HTTP client error: {e}")))?;
        Ok(Self {
            client,
            base_url: format!("{}/storage/v1", SETTINGS.supabase.url),
            service_key: SETTINGS.supabase.service_key.clone(),
            bucket_name: bucket_name.to_string(),
        })
    }

    // -- helpers ------------------------------------------------------------

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("apikey", &self.service_key)
            .bearer_auth(&self.service_key)
    }

    // -- operations ---------------------------------------------------------

    /// Ensure the bucket exists, creating it if necessary.
    pub async fn ensure_bucket_exists(&self) -> Result<(), AppError> {
        let url = format!("{}/bucket/{}", self.base_url, self.bucket_name);

        let resp = self
            .auth(self.client.get(&url))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Storage request failed: {e}")))?;

        if resp.status().is_success() {
            return Ok(());
        }

        let create_url = format!("{}/bucket", self.base_url);
        let resp = self
            .auth(self.client.post(&create_url))
            .json(&CreateBucket {
                name: &self.bucket_name,
                public: false,
            })
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Bucket creation failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            if !body.contains("already exists") {
                return Err(AppError::ExternalService(format!(
                    "Bucket creation failed: {body}"
                )));
            }
        }
        Ok(())
    }

    /// Upload a file and return its public URL.
    pub async fn upload_file(
        &self,
        filename: &str,
        content: Vec<u8>,
        content_type: &str,
        path: Option<&str>,
    ) -> Result<String, AppError> {
        let unique = format!("{}-{filename}", Uuid::new_v4());
        let full_path = path.map_or_else(|| unique.clone(), |p| format!("{p}/{unique}"));

        let url = format!(
            "{}/object/{}/{}",
            self.base_url, self.bucket_name, full_path
        );

        let resp = self
            .auth(self.client.post(&url))
            .header("Content-Type", content_type)
            .body(content)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Upload failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!("Upload failed: {body}")));
        }

        Ok(self.public_url(&full_path))
    }

    /// Build the public URL for a stored object.
    pub fn public_url(&self, path: &str) -> String {
        format!(
            "{}/object/public/{}/{path}",
            self.base_url, self.bucket_name
        )
    }

    /// Delete a file from the bucket.
    pub async fn delete_file(&self, path: &str) -> Result<bool, AppError> {
        let url = format!("{}/object/{}/{path}", self.base_url, self.bucket_name);
        let resp = self
            .auth(self.client.delete(&url))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Delete failed: {e}")))?;
        Ok(resp.status().is_success())
    }

    /// List files under an optional directory prefix.
    pub async fn list_files(&self, path: Option<&str>) -> Result<Vec<FileInfo>, AppError> {
        #[derive(serde::Serialize)]
        struct Req<'a> {
            prefix: &'a str,
        }

        let url = format!("{}/object/list/{}", self.base_url, self.bucket_name);
        let resp = self
            .auth(self.client.post(&url))
            .json(&Req {
                prefix: path.unwrap_or(""),
            })
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("List failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!("List failed: {body}")));
        }

        resp.json()
            .await
            .map_err(|e| AppError::ExternalService(format!("Parse error: {e}")))
    }
}

/// File metadata from a storage listing.
#[derive(Debug, Clone, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub id: Option<String>,
    pub updated_at: Option<String>,
    pub created_at: Option<String>,
    #[serde(default)]
    pub metadata: FileMetadata,
}

/// Size and MIME type from a listing entry.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileMetadata {
    pub size: Option<u64>,
    pub mimetype: Option<String>,
}
