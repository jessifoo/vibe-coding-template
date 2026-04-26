//! Qdrant vector database service.
//!
//! Handles document storage, retrieval, and semantic search via Qdrant.

use crate::config::SETTINGS;
use crate::models::{AppError, DocumentContent, SearchResult};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointId,
    PointStruct, SearchPointsBuilder, UpsertPointsBuilder, Value as QdrantValue,
    VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

const DOCUMENT_PAYLOAD_KEY: &str = "document";
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
    /// * `owner_user_id` - Authenticated user that owns the inserted documents
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
        metadata: Option<&[HashMap<String, JsonValue>]>,
        owner_user_id: &str,
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

        let mut points = Vec::with_capacity(documents.len());
        for (i, ((doc, embedding), id)) in documents
            .iter()
            .zip(embeddings.iter())
            .zip(ids.iter())
            .enumerate()
        {
            let mut payload = HashMap::new();

            // Add document content
            let mut doc_content = HashMap::new();
            doc_content.insert("text".to_string(), JsonValue::String(doc.text.clone()));
            if let Some(title) = &doc.title {
                doc_content.insert("title".to_string(), JsonValue::String(title.clone()));
            }
            let document_json = serde_json::to_string(&doc_content).map_err(|e| {
                AppError::Internal(format!("Failed to serialize document payload: {e}"))
            })?;
            payload.insert(
                DOCUMENT_PAYLOAD_KEY.to_string(),
                QdrantValue::from(document_json),
            );
            payload.insert(
                OWNER_PAYLOAD_KEY.to_string(),
                QdrantValue::from(owner_user_id.to_string()),
            );

            // Add metadata after excluding service-owned payload keys.
            if let Some(meta_list) = metadata {
                if let Some(meta) = meta_list.get(i) {
                    for (key, value) in meta {
                        if is_reserved_payload_key(key) {
                            continue;
                        }

                        payload.insert(key.clone(), QdrantValue::from(value.to_string()));
                    }
                }
            }

            points.push(PointStruct::new(id.clone(), embedding.clone(), payload));
        }

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
    /// * `owner_user_id` - Authenticated user whose documents may be searched
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub async fn search(
        &self,
        query_embedding: &[f32],
        limit: u32,
        filter_params: Option<&HashMap<String, JsonValue>>,
        owner_user_id: &str,
    ) -> Result<Vec<SearchResult>, AppError> {
        // Ensure collection exists
        self.ensure_collection_exists(query_embedding.len() as u64)
            .await?;

        let filter = build_owner_scoped_filter(filter_params, owner_user_id);

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
                    if key == DOCUMENT_PAYLOAD_KEY {
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

    /// Delete documents by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - Document IDs to delete
    /// * `owner_user_id` - Authenticated user whose documents may be deleted
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete(&self, ids: &[String], owner_user_id: &str) -> Result<bool, AppError> {
        if ids.is_empty() {
            return Ok(true);
        }

        let points: Vec<PointId> = ids.iter().map(|id| PointId::from(id.clone())).collect();
        let filter = build_delete_filter(points, owner_user_id);
        let delete_request = DeletePointsBuilder::new(&self.collection_name).points(filter);

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

fn build_owner_scoped_filter(
    filter_params: Option<&HashMap<String, JsonValue>>,
    owner_user_id: &str,
) -> Filter {
    let mut conditions: Vec<Condition> = filter_params
        .into_iter()
        .flat_map(|params| {
            params.iter().filter_map(|(key, value)| {
                if is_reserved_payload_key(key) {
                    None
                } else {
                    Some(metadata_condition(key, value))
                }
            })
        })
        .collect();

    conditions.push(owner_condition(owner_user_id));
    Filter::must(conditions)
}

fn build_delete_filter(points: Vec<PointId>, owner_user_id: &str) -> Filter {
    Filter::must([Condition::has_id(points), owner_condition(owner_user_id)])
}

fn metadata_condition(key: &str, value: &JsonValue) -> Condition {
    Condition::matches(key, value.to_string().trim_matches('"').to_string())
}

fn owner_condition(owner_user_id: &str) -> Condition {
    Condition::matches(OWNER_PAYLOAD_KEY, owner_user_id.to_string())
}

fn is_reserved_payload_key(key: &str) -> bool {
    key == DOCUMENT_PAYLOAD_KEY || key == OWNER_PAYLOAD_KEY
}

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::qdrant::{
        condition::ConditionOneOf, point_id::PointIdOptions, r#match::MatchValue, FieldCondition,
        HasIdCondition,
    };

    fn match_keyword(condition: &Condition, key: &str) -> Option<String> {
        let ConditionOneOf::Field(FieldCondition {
            key: condition_key,
            r#match: Some(match_value),
            ..
        }) = condition.condition_one_of.as_ref()?
        else {
            return None;
        };

        if condition_key != key {
            return None;
        }

        match match_value.match_value.as_ref()? {
            MatchValue::Keyword(value) => Some(value.clone()),
            _ => None,
        }
    }

    #[test]
    fn owner_scoped_filter_always_includes_owner_and_excludes_reserved_metadata() {
        let filter_params = HashMap::from([
            (
                "category".to_string(),
                JsonValue::String("notes".to_string()),
            ),
            (
                OWNER_PAYLOAD_KEY.to_string(),
                JsonValue::String("attacker".to_string()),
            ),
            (
                DOCUMENT_PAYLOAD_KEY.to_string(),
                JsonValue::String("corrupt".to_string()),
            ),
        ]);

        let filter = build_owner_scoped_filter(Some(&filter_params), "user-a");

        assert_eq!(filter.must.len(), 2);
        assert!(filter
            .must
            .iter()
            .any(
                |condition| match_keyword(condition, OWNER_PAYLOAD_KEY).as_deref()
                    == Some("user-a")
            ));
        assert!(filter
            .must
            .iter()
            .any(|condition| match_keyword(condition, "category").as_deref() == Some("notes")));
        assert!(!filter.must.iter().any(|condition| match_keyword(
            condition,
            DOCUMENT_PAYLOAD_KEY
        )
        .is_some()));
    }

    #[test]
    fn delete_filter_requires_requested_ids_and_owner() {
        let filter = build_delete_filter(
            vec![
                PointId::from("8a1d92cc-3d0b-47a5-bc2f-f2f8a96cbe32".to_string()),
                PointId::from("df51fdf8-6142-4059-9416-c77bd14cc339".to_string()),
            ],
            "user-a",
        );

        assert_eq!(filter.must.len(), 2);
        assert!(filter
            .must
            .iter()
            .any(
                |condition| match_keyword(condition, OWNER_PAYLOAD_KEY).as_deref()
                    == Some("user-a")
            ));
        assert!(filter.must.iter().any(|condition| {
            let Some(ConditionOneOf::HasId(HasIdCondition { has_id })) =
                condition.condition_one_of.as_ref()
            else {
                return false;
            };

            has_id.len() == 2
                && has_id.iter().all(|point_id| {
                    matches!(point_id.point_id_options, Some(PointIdOptions::Uuid(_)))
                })
        }));
    }

    #[test]
    fn reserved_payload_keys_are_service_owned() {
        assert!(is_reserved_payload_key(DOCUMENT_PAYLOAD_KEY));
        assert!(is_reserved_payload_key(OWNER_PAYLOAD_KEY));
        assert!(!is_reserved_payload_key("category"));
    }
}
