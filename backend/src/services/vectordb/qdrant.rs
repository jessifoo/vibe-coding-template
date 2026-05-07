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

const OWNER_PAYLOAD_KEY: &str = "_owner_user_id";

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
        owner_user_id: &str,
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
                            if key == OWNER_PAYLOAD_KEY {
                                continue;
                            }

                            payload.insert(
                                key.clone(),
                                qdrant_client::qdrant::Value::from(
                                    qdrant_payload_string_from_json(value),
                                ),
                            );
                        }
                    }
                }
                payload.insert(
                    OWNER_PAYLOAD_KEY.to_string(),
                    qdrant_client::qdrant::Value::from(owner_user_id.to_string()),
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
    /// * `limit` - Maximum number of results to return
    /// * `filter_params` - Optional metadata filters
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub async fn search(
        &self,
        owner_user_id: &str,
        query_embedding: &[f32],
        limit: u32,
        filter_params: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<SearchResult>, AppError> {
        // Ensure collection exists
        self.ensure_collection_exists(query_embedding.len() as u64)
            .await?;

        let filter = scoped_filter(owner_user_id, filter_params);

        let search_builder = SearchPointsBuilder::new(
            &self.collection_name,
            query_embedding.to_vec(),
            limit as u64,
        )
        .with_payload(true)
        .filter(filter);

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
                    } else if key != OWNER_PAYLOAD_KEY {
                        if let Some(s) = extract_string_value(&value) {
                            metadata.insert(key, JsonValue::String(s));
                        }
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

    /// Delete documents by IDs only when they belong to the specified owner.
    ///
    /// IDs that do not belong to the owner are ignored by the filter.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete(&self, owner_user_id: &str, ids: &[String]) -> Result<bool, AppError> {
        if ids.is_empty() {
            return Ok(true);
        }

        let delete_request = DeletePointsBuilder::new(&self.collection_name)
            .points(scoped_delete_filter(owner_user_id, ids));

        self.client
            .delete_points(delete_request)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to delete documents: {e}")))?;

        tracing::info!(
            collection = %self.collection_name,
            owner_id = %owner_user_id,
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

fn qdrant_payload_string_from_json(value: &JsonValue) -> String {
    match value {
        JsonValue::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn owner_condition(owner_user_id: &str) -> Condition {
    Condition::matches(OWNER_PAYLOAD_KEY, owner_user_id.to_string())
}

fn metadata_conditions(filter_params: Option<&HashMap<String, JsonValue>>) -> Vec<Condition> {
    filter_params
        .map(|params| {
            params
                .iter()
                .filter(|(key, _)| key.as_str() != OWNER_PAYLOAD_KEY)
                .map(|(key, value)| {
                    Condition::matches(key.as_str(), qdrant_payload_string_from_json(value))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn scoped_filter(
    owner_user_id: &str,
    filter_params: Option<&HashMap<String, JsonValue>>,
) -> Filter {
    let mut conditions = metadata_conditions(filter_params);
    conditions.push(owner_condition(owner_user_id));
    Filter::must(conditions)
}

fn scoped_delete_filter(owner_user_id: &str, ids: &[String]) -> Filter {
    let points: Vec<PointId> = ids.iter().map(|id| PointId::from(id.clone())).collect();
    Filter::must([Condition::has_id(points), owner_condition(owner_user_id)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::qdrant::condition::ConditionOneOf;
    use qdrant_client::qdrant::r#match::MatchValue;

    #[test]
    fn scoped_filter_always_requires_owner() {
        let filter = scoped_filter("user-a", None);

        assert_eq!(filter.must.len(), 1);
        assert!(has_keyword_condition(
            &filter.must,
            OWNER_PAYLOAD_KEY,
            "user-a"
        ));
    }

    #[test]
    fn scoped_filter_ignores_client_owner_override() {
        let mut metadata = HashMap::new();
        metadata.insert(
            OWNER_PAYLOAD_KEY.to_string(),
            JsonValue::String("user-b".to_string()),
        );
        metadata.insert(
            "category".to_string(),
            JsonValue::String("docs".to_string()),
        );

        let filter = scoped_filter("user-a", Some(&metadata));

        assert_eq!(filter.must.len(), 2);
        assert!(has_keyword_condition(
            &filter.must,
            OWNER_PAYLOAD_KEY,
            "user-a"
        ));
        assert!(!has_keyword_condition(
            &filter.must,
            OWNER_PAYLOAD_KEY,
            "user-b"
        ));
        assert!(has_keyword_condition(&filter.must, "category", "docs"));
    }

    #[test]
    fn scoped_delete_filter_requires_owner_and_ids() {
        let ids = vec![
            "2e07f6c7-3025-4dba-b892-7c1907b5f220".to_string(),
            "565512be-f8b5-48db-a17e-f1469acd9238".to_string(),
        ];

        let filter = scoped_delete_filter("user-a", &ids);

        assert_eq!(filter.must.len(), 2);
        assert!(has_keyword_condition(
            &filter.must,
            OWNER_PAYLOAD_KEY,
            "user-a"
        ));
        assert!(filter
            .must
            .iter()
            .any(|condition| match &condition.condition_one_of {
                Some(ConditionOneOf::HasId(has_id)) => has_id.has_id.len() == ids.len(),
                _ => false,
            }));
    }

    #[test]
    fn string_metadata_is_stored_without_json_quotes() {
        let stored = qdrant_payload_string_from_json(&JsonValue::String("user-123".to_string()));

        assert_eq!(stored, "user-123");
    }

    #[test]
    fn scoped_filter_uses_raw_string_for_path_like_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "path".to_string(),
            JsonValue::String(r"C:\tmp\file.txt".to_string()),
        );

        let filter = scoped_filter("user-a", Some(&metadata));

        assert!(has_keyword_condition(
            &filter.must,
            "path",
            r"C:\tmp\file.txt",
        ));
    }

    fn has_keyword_condition(conditions: &[Condition], key: &str, value: &str) -> bool {
        conditions
            .iter()
            .any(|condition| match &condition.condition_one_of {
                Some(ConditionOneOf::Field(field)) if field.key == key => field
                    .r#match
                    .as_ref()
                    .and_then(|field_match| field_match.match_value.as_ref())
                    .is_some_and(|match_value| match match_value {
                        MatchValue::Keyword(keyword) => keyword == value,
                        _ => false,
                    }),
                _ => false,
            })
    }
}
