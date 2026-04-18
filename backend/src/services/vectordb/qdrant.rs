//! Qdrant vector database service.
//!
//! Handles document storage, retrieval, and semantic search via Qdrant.

use crate::config::SETTINGS;
use crate::models::{AppError, DocumentContent, SearchResult};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointId,
    PointStruct, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

const OWNER_ID_FIELD: &str = "owner_id";

/// Service for interacting with Qdrant vector database.
pub struct QdrantService {
    client: Qdrant,
    collection_name: String,
}

impl QdrantService {
    /// Create a new Qdrant service instance.
    ///
    /// If no URL is configured, uses an in-memory instance.
    ///
    /// # Errors
    ///
    /// Returns an error if connection to Qdrant fails.
    pub async fn new() -> Result<Self, AppError> {
        let collection_name = SETTINGS.qdrant.collection_name.clone();

        let client = if let Some(url) = &SETTINGS.qdrant.url {
            let mut builder = Qdrant::from_url(url);

            if let Some(api_key) = &SETTINGS.qdrant.api_key {
                builder = builder.api_key(api_key.clone());
            }

            builder
                .build()
                .map_err(|e| AppError::Configuration(format!("Failed to connect to Qdrant: {e}")))?
        } else {
            // In-memory instance for development/testing
            Qdrant::from_url("http://localhost:6334")
                .build()
                .map_err(|e| {
                    AppError::Configuration(format!("Failed to create Qdrant client: {e}"))
                })?
        };

        Ok(Self {
            client,
            collection_name,
        })
    }

    /// Ensure the collection exists, creating it if necessary.
    ///
    /// # Arguments
    ///
    /// * `vector_size` - Size of embedding vectors (default: 1536 for OpenAI)
    ///
    /// # Errors
    ///
    /// Returns an error if collection creation fails.
    pub async fn ensure_collection_exists(&self, vector_size: u64) -> Result<(), AppError> {
        // Check if collection exists
        let collections =
            self.client.list_collections().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to list collections: {e}"))
            })?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == self.collection_name);

        if !exists {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.collection_name)
                        .vectors_config(VectorParamsBuilder::new(vector_size, Distance::Cosine)),
                )
                .await
                .map_err(|e| {
                    AppError::ExternalService(format!("Failed to create collection: {e}"))
                })?;

            tracing::info!(
                collection = %self.collection_name,
                vector_size = vector_size,
                "Created new Qdrant collection"
            );
        }

        Ok(())
    }

    /// Add documents with their embeddings to the vector database.
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents with text and optional title
    /// * `embeddings` - Embedding vectors for each document
    /// * `owner_id` - Authenticated user ID used for tenant isolation
    /// * `metadata` - Optional metadata for each document
    ///
    /// # Returns
    ///
    /// List of generated document IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the number of documents doesn't match embeddings.
    pub async fn add_documents(
        &self,
        documents: &[DocumentData],
        embeddings: &[Vec<f32>],
        owner_id: &str,
        metadata: Option<&[HashMap<String, JsonValue>]>,
    ) -> Result<Vec<String>, AppError> {
        if documents.len() != embeddings.len() {
            return Err(AppError::BadRequest(format!(
                "Number of documents ({}) must match number of embeddings ({})",
                documents.len(),
                embeddings.len()
            )));
        }

        if documents.is_empty() {
            return Err(AppError::BadRequest("No documents provided".to_string()));
        }

        // Ensure collection exists with appropriate vector size
        let vector_size = embeddings.first().map(|e| e.len() as u64).unwrap_or(1536);
        self.ensure_collection_exists(vector_size).await?;

        // Generate IDs and create points
        let ids: Vec<String> = (0..documents.len())
            .map(|_| Uuid::new_v4().to_string())
            .collect();

        let points: Vec<PointStruct> = documents
            .iter()
            .zip(embeddings.iter())
            .zip(ids.iter())
            .enumerate()
            .map(|(i, ((doc, embedding), id))| {
                let mut payload = HashMap::new();

                // Add document content
                let mut doc_content = HashMap::new();
                doc_content.insert("text".to_string(), JsonValue::String(doc.text.clone()));
                if let Some(title) = &doc.title {
                    doc_content.insert("title".to_string(), JsonValue::String(title.clone()));
                }
                payload.insert(
                    "document".to_string(),
                    qdrant_client::qdrant::Value::from(
                        serde_json::to_string(&doc_content).unwrap_or_default(),
                    ),
                );

                // Add metadata
                if let Some(meta_list) = metadata {
                    if let Some(meta) = meta_list.get(i) {
                        for (key, value) in meta {
                            payload.insert(
                                key.clone(),
                                qdrant_client::qdrant::Value::from(value.to_string()),
                            );
                        }
                    }
                }

                // Always stamp owner_id last so user-provided metadata cannot override it.
                payload.insert(
                    OWNER_ID_FIELD.to_string(),
                    qdrant_client::qdrant::Value::from(owner_id.to_string()),
                );

                PointStruct::new(id.clone(), embedding.clone(), payload)
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to add documents: {e}")))?;

        tracing::info!(
            collection = %self.collection_name,
            count = ids.len(),
            "Added documents to Qdrant"
        );

        Ok(ids)
    }

    /// Search for documents similar to the query embedding.
    ///
    /// # Arguments
    ///
    /// * `query_embedding` - Embedding vector of the query
    /// * `owner_id` - Authenticated user ID used for tenant isolation
    /// * `limit` - Maximum number of results to return
    /// * `filter_params` - Optional metadata filters
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub async fn search(
        &self,
        query_embedding: &[f32],
        owner_id: &str,
        limit: u32,
        filter_params: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<SearchResult>, AppError> {
        // Ensure collection exists
        self.ensure_collection_exists(query_embedding.len() as u64)
            .await?;

        // Always enforce owner scoping to prevent cross-tenant reads.
        let filter = build_search_filter(owner_id, filter_params);

        let mut search_builder = SearchPointsBuilder::new(
            &self.collection_name,
            query_embedding.to_vec(),
            limit as u64,
        )
        .with_payload(true);

        search_builder = search_builder.filter(filter);

        let search_result = self
            .client
            .search_points(search_builder)
            .await
            .map_err(|e| AppError::ExternalService(format!("Search failed: {e}")))?;

        let results = search_result
            .result
            .into_iter()
            .map(|point| {
                let id = match point.id {
                    Some(PointId {
                        point_id_options: Some(id),
                    }) => match id {
                        qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid) => uuid,
                        qdrant_client::qdrant::point_id::PointIdOptions::Num(num) => {
                            num.to_string()
                        }
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
                        // Parse document JSON
                        if let Some(s) = extract_string_value(&value) {
                            if let Ok(doc_map) = serde_json::from_str::<HashMap<String, String>>(&s)
                            {
                                if let Some(text) = doc_map.get("text") {
                                    document.text = text.clone();
                                }
                                document.title = doc_map.get("title").cloned();
                            }
                        }
                    } else if let Some(s) = extract_string_value(&value) {
                        metadata.insert(key, JsonValue::String(s));
                    }
                }

                SearchResult {
                    id,
                    score: point.score,
                    document,
                    metadata,
                }
            })
            .collect();

        Ok(results)
    }

    /// Delete documents by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - Document IDs to delete
    /// * `owner_id` - Authenticated user ID used for tenant isolation
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete(&self, ids: &[String], owner_id: &str) -> Result<bool, AppError> {
        if ids.is_empty() {
            return Ok(true);
        }

        // Restrict deletes to the caller's own documents only.
        let delete_filter = build_delete_filter(owner_id, ids);
        let delete_request = DeletePointsBuilder::new(&self.collection_name).points(delete_filter);

        self.client
            .delete_points(delete_request)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to delete documents: {e}")))?;

        tracing::info!(
            collection = %self.collection_name,
            count = ids.len(),
            "Deleted documents from Qdrant"
        );

        Ok(true)
    }
}

/// Document data for storage.
#[derive(Debug, Clone)]
pub struct DocumentData {
    /// Document text content
    pub text: String,

    /// Optional document title
    pub title: Option<String>,
}

/// Extract string value from Qdrant Value.
fn extract_string_value(value: &qdrant_client::qdrant::Value) -> Option<String> {
    match &value.kind {
        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
        _ => None,
    }
}

fn build_owner_condition(owner_id: &str) -> Condition {
    Condition::matches(OWNER_ID_FIELD, owner_id.to_string())
}

fn build_search_filter(
    owner_id: &str,
    filter_params: Option<&HashMap<String, JsonValue>>,
) -> Filter {
    let mut conditions: Vec<Condition> = vec![build_owner_condition(owner_id)];

    if let Some(params) = filter_params {
        conditions.extend(params.iter().map(|(key, value)| {
            Condition::matches(
                key.as_str(),
                value.to_string().trim_matches('"').to_string(),
            )
        }));
    }

    Filter::must(conditions)
}

fn build_delete_filter(owner_id: &str, ids: &[String]) -> Filter {
    Filter::must([
        build_owner_condition(owner_id),
        Condition::has_id(ids.iter().cloned()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::qdrant::{condition::ConditionOneOf, r#match::MatchValue};

    #[test]
    fn search_filter_always_includes_owner_condition() {
        let filter = build_search_filter("user_123", None);

        assert_eq!(filter.must.len(), 1);
        assert!(filter
            .must
            .iter()
            .any(|condition| is_owner_condition(condition, "user_123")));
    }

    #[test]
    fn search_filter_merges_owner_and_request_filters() {
        let mut params = HashMap::new();
        params.insert("source".to_string(), JsonValue::String("kb".to_string()));

        let filter = build_search_filter("user_123", Some(&params));

        assert_eq!(filter.must.len(), 2);
        assert!(filter
            .must
            .iter()
            .any(|condition| is_owner_condition(condition, "user_123")));
        assert!(filter
            .must
            .iter()
            .any(|condition| match &condition.condition_one_of {
                Some(ConditionOneOf::Field(field)) => field.key == "source",
                _ => false,
            }));
    }

    #[test]
    fn delete_filter_requires_ids_and_owner() {
        let ids = vec!["doc-1".to_string(), "doc-2".to_string()];
        let filter = build_delete_filter("user_123", &ids);

        assert_eq!(filter.must.len(), 2);
        assert!(filter
            .must
            .iter()
            .any(|condition| is_owner_condition(condition, "user_123")));
        assert!(filter.must.iter().any(|condition| {
            matches!(condition.condition_one_of, Some(ConditionOneOf::HasId(_)))
        }));
    }

    fn is_owner_condition(condition: &Condition, expected_owner: &str) -> bool {
        match &condition.condition_one_of {
            Some(ConditionOneOf::Field(field)) => {
                if field.key != OWNER_ID_FIELD {
                    return false;
                }

                match &field.r#match {
                    Some(r#match) => match &r#match.match_value {
                        Some(MatchValue::Keyword(value)) | Some(MatchValue::Text(value)) => {
                            value == expected_owner
                        }
                        _ => false,
                    },
                    None => false,
                }
            }
            _ => false,
        }
    }
}
