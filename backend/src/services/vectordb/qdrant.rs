//! Qdrant vector-database service.

use std::collections::HashMap;

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointId,
    PointStruct, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::config::SETTINGS;
use crate::models::{AppError, DocumentContent, SearchResult};

/// Client for Qdrant document storage and semantic search.
pub struct QdrantService {
    client: Qdrant,
    collection: String,
}

impl QdrantService {
    /// Connect to Qdrant (remote or local fallback).
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Configuration`] on connection failure.
    pub fn new() -> Result<Self, AppError> {
        let collection = SETTINGS.qdrant.collection_name.clone();

        let client = match &SETTINGS.qdrant.url {
            Some(url) => {
                let mut builder = Qdrant::from_url(url);
                if let Some(key) = &SETTINGS.qdrant.api_key {
                    builder = builder.api_key(key as &str);
                }
                builder
                    .build()
                    .map_err(|e| AppError::Configuration(format!("Qdrant connect error: {e}")))?
            }
            None => Qdrant::from_url("http://localhost:6334")
                .build()
                .map_err(|e| AppError::Configuration(format!("Qdrant client error: {e}")))?,
        };

        Ok(Self { client, collection })
    }

    /// Ensure the collection exists (creates it if missing).
    pub async fn ensure_collection(&self, vector_size: u64) -> Result<(), AppError> {
        let collections = self
            .client
            .list_collections()
            .await
            .map_err(|e| AppError::ExternalService(format!("List collections failed: {e}")))?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == self.collection);
        if exists {
            return Ok(());
        }

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(VectorParamsBuilder::new(vector_size, Distance::Cosine)),
            )
            .await
            .map_err(|e| AppError::ExternalService(format!("Create collection failed: {e}")))?;

        tracing::info!(collection = %self.collection, vector_size, "Created Qdrant collection");
        Ok(())
    }

    /// Store documents with pre-computed embeddings.
    pub async fn add_documents(
        &self,
        docs: &[DocumentData],
        embeddings: &[Vec<f32>],
        metadata: Option<&[HashMap<String, JsonValue>]>,
    ) -> Result<Vec<String>, AppError> {
        if docs.len() != embeddings.len() {
            return Err(AppError::BadRequest(format!(
                "Document count ({}) must match embedding count ({})",
                docs.len(),
                embeddings.len(),
            )));
        }
        if docs.is_empty() {
            return Err(AppError::BadRequest("No documents provided".into()));
        }

        let vector_size = embeddings.first().map_or(1536, |e| e.len() as u64);
        self.ensure_collection(vector_size).await?;

        let ids: Vec<String> = (0..docs.len())
            .map(|_| Uuid::new_v4().to_string())
            .collect();

        let points: Vec<PointStruct> = docs
            .iter()
            .zip(embeddings)
            .zip(&ids)
            .enumerate()
            .map(|(i, ((doc, emb), id))| build_point(id, doc, emb, metadata, i))
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, points))
            .await
            .map_err(|e| AppError::ExternalService(format!("Upsert failed: {e}")))?;

        tracing::info!(collection = %self.collection, count = ids.len(), "Documents added");
        Ok(ids)
    }

    /// Semantic search using a pre-computed query embedding.
    pub async fn search(
        &self,
        query_embedding: &[f32],
        limit: u32,
        filter_params: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<SearchResult>, AppError> {
        self.ensure_collection(query_embedding.len() as u64).await?;

        let filter = filter_params.map(|params| {
            let conditions: Vec<Condition> = params
                .iter()
                .map(|(k, v)| {
                    Condition::matches(k.as_str(), v.to_string().trim_matches('"').to_string())
                })
                .collect();
            Filter::must(conditions)
        });

        let mut builder =
            SearchPointsBuilder::new(&self.collection, query_embedding.to_vec(), u64::from(limit))
                .with_payload(true);

        if let Some(f) = filter {
            builder = builder.filter(f);
        }

        let result = self
            .client
            .search_points(builder)
            .await
            .map_err(|e| AppError::ExternalService(format!("Search failed: {e}")))?;

        Ok(result.result.into_iter().map(parse_search_point).collect())
    }

    /// Delete documents by ID.
    pub async fn delete(&self, ids: &[String]) -> Result<bool, AppError> {
        if ids.is_empty() {
            return Ok(true);
        }

        let points: Vec<PointId> = ids.iter().map(|id| PointId::from(id.clone())).collect();

        self.client
            .delete_points(DeletePointsBuilder::new(&self.collection).points(points))
            .await
            .map_err(|e| AppError::ExternalService(format!("Delete failed: {e}")))?;

        tracing::info!(collection = %self.collection, count = ids.len(), "Documents deleted");
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Lightweight document payload for storage.
#[derive(Debug, Clone)]
pub struct DocumentData {
    pub text: String,
    pub title: Option<String>,
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn build_point(
    id: &str,
    doc: &DocumentData,
    embedding: &[f32],
    metadata: Option<&[HashMap<String, JsonValue>]>,
    index: usize,
) -> PointStruct {
    let mut payload = HashMap::new();

    // Serialise the document content into a single JSON string field.
    let mut content = HashMap::new();
    content.insert("text".to_string(), JsonValue::String(doc.text.clone()));
    if let Some(title) = &doc.title {
        content.insert("title".to_string(), JsonValue::String(title.clone()));
    }
    payload.insert(
        "document".to_string(),
        qdrant_client::qdrant::Value::from(serde_json::to_string(&content).unwrap_or_default()),
    );

    // Flatten metadata into top-level payload fields.
    if let Some(meta_list) = metadata {
        if let Some(meta) = meta_list.get(index) {
            for (k, v) in meta {
                payload.insert(k.clone(), qdrant_client::qdrant::Value::from(v.to_string()));
            }
        }
    }

    PointStruct::new(id.to_string(), embedding.to_vec(), payload)
}

fn parse_search_point(point: qdrant_client::qdrant::ScoredPoint) -> SearchResult {
    let id = match point.id {
        Some(PointId {
            point_id_options: Some(opt),
        }) => match opt {
            qdrant_client::qdrant::point_id::PointIdOptions::Uuid(u) => u,
            qdrant_client::qdrant::point_id::PointIdOptions::Num(n) => n.to_string(),
        },
        _ => String::new(),
    };

    let mut metadata = HashMap::new();
    let mut document = DocumentContent {
        text: String::new(),
        title: None,
    };

    for (key, value) in point.payload {
        if key == "document" {
            if let Some(s) = extract_string(&value) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&s) {
                    if let Some(t) = map.get("text") {
                        document.text.clone_from(t);
                    }
                    document.title = map.get("title").cloned();
                }
            }
        } else if let Some(s) = extract_string(&value) {
            metadata.insert(key, JsonValue::String(s));
        }
    }

    SearchResult {
        id,
        score: point.score,
        document,
        metadata,
    }
}

fn extract_string(value: &qdrant_client::qdrant::Value) -> Option<String> {
    match &value.kind {
        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
        _ => None,
    }
}
