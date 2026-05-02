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
                let payload = build_point_payload(
                    doc,
                    metadata.and_then(|meta_list| meta_list.get(i)),
                    owner_user_id,
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

        let filter = build_search_filter(owner_user_id, filter_params);

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

                let mut document = DocumentContent {
                    text: String::new(),
                    title: None,
                };
                let mut metadata_payload = HashMap::new();

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
                    } else {
                        metadata_payload.insert(key, value);
                    }
                }
                let metadata = metadata_from_payload(metadata_payload);

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
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete(&self, owner_user_id: &str, ids: &[String]) -> Result<bool, AppError> {
        if ids.is_empty() {
            return Ok(true);
        }

        let delete_request = DeletePointsBuilder::new(&self.collection_name)
            .points(build_delete_filter(owner_user_id, ids));

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

fn build_point_payload(
    doc: &DocumentData,
    metadata: Option<&HashMap<String, JsonValue>>,
    owner_user_id: &str,
) -> HashMap<String, qdrant_client::qdrant::Value> {
    let mut payload = HashMap::new();

    let mut doc_content = serde_json::Map::new();
    doc_content.insert("text".to_string(), JsonValue::String(doc.text.clone()));
    if let Some(title) = &doc.title {
        doc_content.insert("title".to_string(), JsonValue::String(title.clone()));
    }
    let document_json = JsonValue::Object(doc_content).to_string();
    payload.insert(
        "document".to_string(),
        qdrant_client::qdrant::Value::from(document_json),
    );

    if let Some(meta) = metadata {
        for (key, value) in meta {
            if key == OWNER_PAYLOAD_KEY {
                continue;
            }
            payload.insert(
                key.clone(),
                qdrant_client::qdrant::Value::from(value.to_string()),
            );
        }
    }

    payload.insert(
        OWNER_PAYLOAD_KEY.to_string(),
        qdrant_client::qdrant::Value::from(owner_user_id),
    );
    payload
}

fn build_search_filter(
    owner_user_id: &str,
    filter_params: Option<&HashMap<String, JsonValue>>,
) -> Filter {
    let mut conditions = vec![Condition::matches(
        OWNER_PAYLOAD_KEY,
        owner_user_id.to_string(),
    )];

    if let Some(params) = filter_params {
        conditions.extend(params.iter().filter_map(|(key, value)| {
            if key == OWNER_PAYLOAD_KEY {
                return None;
            }
            Some(Condition::matches(
                key.as_str(),
                value.to_string().trim_matches('"').to_string(),
            ))
        }));
    }

    Filter::must(conditions)
}

fn build_delete_filter(owner_user_id: &str, ids: &[String]) -> Filter {
    Filter::must([
        Condition::matches(OWNER_PAYLOAD_KEY, owner_user_id.to_string()),
        Condition::has_id(ids.iter().cloned()),
    ])
}

fn metadata_from_payload(
    payload: HashMap<String, qdrant_client::qdrant::Value>,
) -> HashMap<String, JsonValue> {
    payload
        .into_iter()
        .filter_map(|(key, value)| {
            if key == OWNER_PAYLOAD_KEY {
                return None;
            }
            extract_string_value(&value).map(|s| (key, JsonValue::String(s)))
        })
        .collect()
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

    #[test]
    fn point_payload_stores_owner_after_metadata() {
        let doc = DocumentData {
            text: "secret document".to_string(),
            title: Some("Secret".to_string()),
        };
        let mut metadata = HashMap::new();
        metadata.insert(
            OWNER_PAYLOAD_KEY.to_string(),
            JsonValue::String("attacker-user".to_string()),
        );
        metadata.insert(
            "category".to_string(),
            JsonValue::String("notes".to_string()),
        );

        let payload = build_point_payload(&doc, Some(&metadata), "real-user");

        assert_eq!(
            extract_string_value(payload.get(OWNER_PAYLOAD_KEY).expect("owner payload")),
            Some("real-user".to_string())
        );
        assert_eq!(
            extract_string_value(payload.get("category").expect("category payload")),
            Some("\"notes\"".to_string())
        );
    }

    #[test]
    fn search_filter_always_requires_owner() {
        let mut metadata_filter = HashMap::new();
        metadata_filter.insert(
            "category".to_string(),
            JsonValue::String("notes".to_string()),
        );

        let filter = build_search_filter("owner-user", Some(&metadata_filter));

        assert_eq!(filter.must.len(), 2);
        assert!(filter_contains_match(
            &filter,
            OWNER_PAYLOAD_KEY,
            "owner-user"
        ));
        assert!(filter_contains_match(&filter, "category", "notes"));
    }

    #[test]
    fn delete_filter_requires_owner_and_ids() {
        let ids = vec!["11111111-1111-4111-8111-111111111111".to_string()];

        let filter = build_delete_filter("owner-user", &ids);

        assert_eq!(filter.must.len(), 2);
        assert!(filter_contains_match(
            &filter,
            OWNER_PAYLOAD_KEY,
            "owner-user"
        ));
        assert!(filter_contains_has_id(&filter, &ids));
    }

    #[test]
    fn owner_payload_is_not_returned_as_metadata() {
        let mut payload = HashMap::new();
        payload.insert(
            OWNER_PAYLOAD_KEY.to_string(),
            qdrant_client::qdrant::Value::from("owner-user"),
        );
        payload.insert(
            "category".to_string(),
            qdrant_client::qdrant::Value::from("notes"),
        );

        let metadata = metadata_from_payload(payload);

        assert!(!metadata.contains_key(OWNER_PAYLOAD_KEY));
        assert_eq!(
            metadata.get("category"),
            Some(&JsonValue::String("notes".to_string()))
        );
    }

    fn filter_contains_match(filter: &Filter, key: &str, value: &str) -> bool {
        filter.must.iter().any(|condition| {
            let Some(ConditionOneOf::Field(field)) = &condition.condition_one_of else {
                return false;
            };
            if field.key != key {
                return false;
            }
            let Some(qdrant_client::qdrant::Match {
                match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(keyword)),
            }) = &field.r#match
            else {
                return false;
            };

            keyword == value
        })
    }

    fn filter_contains_has_id(filter: &Filter, ids: &[String]) -> bool {
        filter.must.iter().any(|condition| {
            let Some(ConditionOneOf::HasId(has_id)) = &condition.condition_one_of else {
                return false;
            };

            has_id.has_id.len() == ids.len()
                && has_id
                    .has_id
                    .iter()
                    .zip(ids.iter())
                    .all(|(point_id, expected)| match &point_id.point_id_options {
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid)) => {
                            uuid == expected
                        }
                        _ => false,
                    })
        })
    }
}
