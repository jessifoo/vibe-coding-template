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

const OWNER_USER_ID_FIELD: &str = "owner_user_id";

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
    /// * `user_id` - Authenticated user ID for ownership scoping
    /// * `documents` - Documents with text and optional title
    /// * `embeddings` - Embedding vectors for each document
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
        user_id: &str,
        documents: &[DocumentData],
        embeddings: &[Vec<f32>],
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

                payload.insert(
                    OWNER_USER_ID_FIELD.to_string(),
                    qdrant_client::qdrant::Value::from(user_id.to_string()),
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
    /// * `user_id` - Authenticated user ID for ownership scoping
    /// * `query_embedding` - Embedding vector of the query
    /// * `limit` - Maximum number of results to return
    /// * `filter_params` - Optional metadata filters
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub async fn search(
        &self,
        user_id: &str,
        query_embedding: &[f32],
        limit: u32,
        filter_params: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<SearchResult>, AppError> {
        // Ensure collection exists
        self.ensure_collection_exists(query_embedding.len() as u64)
            .await?;

        let filter = build_search_filter(user_id, filter_params);

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
    /// * `user_id` - Authenticated user ID for ownership scoping
    /// * `ids` - Document IDs to delete
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete(&self, user_id: &str, ids: &[String]) -> Result<bool, AppError> {
        if ids.is_empty() {
            return Ok(true);
        }

        let delete_filter = build_delete_filter(user_id, ids);
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

fn metadata_value_to_match_string(value: &JsonValue) -> String {
    value.to_string().trim_matches('"').to_string()
}

fn build_search_filter(
    user_id: &str,
    filter_params: Option<&HashMap<String, JsonValue>>,
) -> Filter {
    let mut conditions = vec![Condition::matches(OWNER_USER_ID_FIELD, user_id.to_string())];

    if let Some(params) = filter_params {
        let metadata_conditions = params.iter().map(|(key, value)| {
            Condition::matches(key.as_str(), metadata_value_to_match_string(value))
        });
        conditions.extend(metadata_conditions);
    }

    Filter::must(conditions)
}

fn build_delete_filter(user_id: &str, ids: &[String]) -> Filter {
    let point_ids: Vec<PointId> = ids.iter().cloned().map(PointId::from).collect();
    Filter::must([
        Condition::matches(OWNER_USER_ID_FIELD, user_id.to_string()),
        Condition::has_id(point_ids),
    ])
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

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::qdrant::condition::ConditionOneOf;

    fn condition_contains_match(condition: &Condition, key: &str, expected: &str) -> bool {
        match &condition.condition_one_of {
            Some(ConditionOneOf::Field(field_condition)) => {
                if field_condition.key != key {
                    return false;
                }

                field_condition
                    .r#match
                    .as_ref()
                    .and_then(|m| m.match_value.as_ref())
                    .is_some_and(|match_value| match match_value {
                        qdrant_client::qdrant::r#match::MatchValue::Keyword(value) => {
                            value == expected
                        }
                        qdrant_client::qdrant::r#match::MatchValue::Text(value) => {
                            value == expected
                        }
                        _ => false,
                    })
            }
            _ => false,
        }
    }

    fn condition_is_has_id(condition: &Condition, expected_count: usize) -> bool {
        match &condition.condition_one_of {
            Some(ConditionOneOf::HasId(has_id_condition)) => {
                has_id_condition.has_id.len() == expected_count
            }
            _ => false,
        }
    }

    #[test]
    fn search_filter_always_scopes_by_owner_even_without_metadata() {
        let filter = build_search_filter("user-123", None);
        assert_eq!(filter.must.len(), 1);
        assert!(condition_contains_match(
            &filter.must[0],
            OWNER_USER_ID_FIELD,
            "user-123"
        ));
    }

    #[test]
    fn search_filter_includes_owner_and_metadata_conditions() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "topic".to_string(),
            JsonValue::String("billing".to_string()),
        );

        let filter = build_search_filter("user-123", Some(&metadata));
        assert_eq!(filter.must.len(), 2);
        assert!(condition_contains_match(
            &filter.must[0],
            OWNER_USER_ID_FIELD,
            "user-123"
        ));
        assert!(condition_contains_match(
            &filter.must[1],
            "topic",
            "billing"
        ));
    }

    #[test]
    fn delete_filter_requires_owner_and_ids() {
        let ids = vec!["doc-1".to_string(), "doc-2".to_string()];
        let filter = build_delete_filter("user-123", &ids);
        assert_eq!(filter.must.len(), 2);
        assert!(condition_contains_match(
            &filter.must[0],
            OWNER_USER_ID_FIELD,
            "user-123"
        ));
        assert!(condition_is_has_id(&filter.must[1], 2));
    }
}
