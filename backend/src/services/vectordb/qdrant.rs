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

const OWNER_USER_ID_PAYLOAD_KEY: &str = "_owner_user_id";

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
                let metadata_for_doc = metadata.and_then(|meta_list| meta_list.get(i));
                let payload = build_point_payload(doc, metadata_for_doc, owner_user_id)?;

                Ok(PointStruct::new(id.clone(), embedding.clone(), payload))
            })
            .collect::<Result<Vec<_>, AppError>>()?;

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
                    } else if key != OWNER_USER_ID_PAYLOAD_KEY {
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
) -> Result<HashMap<String, qdrant_client::qdrant::Value>, AppError> {
    let mut payload = HashMap::new();

    let mut doc_content = HashMap::new();
    doc_content.insert("text".to_string(), JsonValue::String(doc.text.clone()));
    if let Some(title) = &doc.title {
        doc_content.insert("title".to_string(), JsonValue::String(title.clone()));
    }
    let document_json = serde_json::to_string(&doc_content)
        .map_err(|e| AppError::Internal(format!("Failed to serialize document payload: {e}")))?;
    payload.insert(
        "document".to_string(),
        qdrant_client::qdrant::Value::from(document_json),
    );

    if let Some(meta) = metadata {
        for (key, value) in meta {
            if key == OWNER_USER_ID_PAYLOAD_KEY {
                continue;
            }
            payload.insert(
                key.clone(),
                qdrant_client::qdrant::Value::from(value.to_string()),
            );
        }
    }

    payload.insert(
        OWNER_USER_ID_PAYLOAD_KEY.to_string(),
        qdrant_client::qdrant::Value::from(owner_user_id.to_string()),
    );

    Ok(payload)
}

fn build_search_filter(
    owner_user_id: &str,
    filter_params: Option<&HashMap<String, JsonValue>>,
) -> Filter {
    let mut conditions = vec![owner_condition(owner_user_id)];

    if let Some(params) = filter_params {
        conditions.extend(params.iter().filter_map(|(key, value)| {
            if key == OWNER_USER_ID_PAYLOAD_KEY {
                None
            } else {
                Some(Condition::matches(
                    key.as_str(),
                    metadata_match_value(value),
                ))
            }
        }));
    }

    Filter::must(conditions)
}

fn build_delete_filter(owner_user_id: &str, ids: &[String]) -> Filter {
    let points: Vec<PointId> = ids.iter().map(|id| PointId::from(id.clone())).collect();

    Filter::must([owner_condition(owner_user_id), Condition::has_id(points)])
}

fn owner_condition(owner_user_id: &str) -> Condition {
    Condition::matches(OWNER_USER_ID_PAYLOAD_KEY, owner_user_id.to_string())
}

fn metadata_match_value(value: &JsonValue) -> String {
    value.to_string().trim_matches('"').to_string()
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

    fn field_match_value(condition: &Condition) -> Option<(&str, &str)> {
        match condition.condition_one_of.as_ref()? {
            ConditionOneOf::Field(field) => {
                let match_value = field.r#match.as_ref()?.match_value.as_ref()?;
                match match_value {
                    qdrant_client::qdrant::r#match::MatchValue::Keyword(value) => {
                        Some((field.key.as_str(), value.as_str()))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[test]
    fn search_filter_always_scopes_to_authenticated_owner() {
        let mut requested_filters = HashMap::new();
        requested_filters.insert(
            "category".to_string(),
            JsonValue::String("docs".to_string()),
        );
        requested_filters.insert(
            OWNER_USER_ID_PAYLOAD_KEY.to_string(),
            JsonValue::String("attacker-user".to_string()),
        );

        let filter = build_search_filter("real-user", Some(&requested_filters));

        assert!(filter
            .must
            .iter()
            .any(|condition| field_match_value(condition)
                == Some((OWNER_USER_ID_PAYLOAD_KEY, "real-user"))));
        assert!(filter
            .must
            .iter()
            .any(|condition| field_match_value(condition) == Some(("category", "docs"))));
        assert!(!filter
            .must
            .iter()
            .any(|condition| field_match_value(condition)
                == Some((OWNER_USER_ID_PAYLOAD_KEY, "attacker-user"))));
    }

    #[test]
    fn delete_filter_requires_owner_and_requested_ids() {
        let ids = vec!["doc-a".to_string(), "doc-b".to_string()];

        let filter = build_delete_filter("real-user", &ids);

        assert!(filter
            .must
            .iter()
            .any(|condition| field_match_value(condition)
                == Some((OWNER_USER_ID_PAYLOAD_KEY, "real-user"))));
        assert!(filter
            .must
            .iter()
            .any(|condition| matches!(condition.condition_one_of, Some(ConditionOneOf::HasId(_)))));
    }

    #[test]
    fn owner_payload_overrides_reserved_metadata_key() {
        let mut metadata = HashMap::new();
        metadata.insert(
            OWNER_USER_ID_PAYLOAD_KEY.to_string(),
            JsonValue::String("attacker-user".to_string()),
        );

        let payload = build_point_payload(
            &DocumentData {
                text: "body".to_string(),
                title: None,
            },
            Some(&metadata),
            "real-user",
        )
        .expect("test document serializes");

        assert_eq!(
            payload
                .get(OWNER_USER_ID_PAYLOAD_KEY)
                .and_then(extract_string_value),
            Some("real-user".to_string())
        );
    }
}
