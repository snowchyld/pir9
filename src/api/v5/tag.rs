//! Tag API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{
    DelayProfileRepository, DownloadClientRepository, IndexerRepository, NotificationRepository,
    TagRepository,
};
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagResource {
    #[serde(default)]
    pub id: i32,
    pub label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDetailsResource {
    pub id: i32,
    pub label: String,
    pub delay_profile_ids: Vec<i32>,
    pub import_list_ids: Vec<i32>,
    pub notification_ids: Vec<i32>,
    pub restriction_ids: Vec<i32>,
    pub indexer_ids: Vec<i32>,
    pub download_client_ids: Vec<i32>,
    pub auto_tag_ids: Vec<i32>,
    pub series_ids: Vec<i32>,
}

/// GET /api/v5/tag - List all tags
pub async fn get_tags(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TagResource>>, TagError> {
    let repo = TagRepository::new(state.db.clone());

    let tags = repo
        .get_all()
        .await
        .map_err(|e| TagError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let resources: Vec<TagResource> = tags
        .into_iter()
        .map(|(id, label)| TagResource {
            id: id as i32,
            label,
        })
        .collect();

    Ok(Json(resources))
}

/// GET /api/v5/tag/{id} - Get a specific tag
pub async fn get_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<TagResource>, TagError> {
    let repo = TagRepository::new(state.db.clone());

    let tags = repo
        .get_all()
        .await
        .map_err(|e| TagError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let tag = tags
        .into_iter()
        .find(|(tag_id, _)| *tag_id == id as i64)
        .map(|(id, label)| TagResource {
            id: id as i32,
            label,
        })
        .ok_or(TagError::NotFound)?;

    Ok(Json(tag))
}

/// POST /api/v5/tag - Create a new tag
pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TagResource>,
) -> Result<Json<TagResource>, TagError> {
    if body.label.trim().is_empty() {
        return Err(TagError::Validation("Label is required".to_string()));
    }

    let repo = TagRepository::new(state.db.clone());

    let id = repo
        .insert(&body.label)
        .await
        .map_err(|e| TagError::Internal(format!("Failed to create tag: {}", e)))?;

    tracing::info!("Created tag: id={}, label={}", id, body.label);

    Ok(Json(TagResource {
        id: id as i32,
        label: body.label,
    }))
}

/// PUT /api/v5/tag/{id} - Update a tag
pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<TagResource>,
) -> Result<Json<TagResource>, TagError> {
    if body.label.trim().is_empty() {
        return Err(TagError::Validation("Label is required".to_string()));
    }

    let repo = TagRepository::new(state.db.clone());

    // Verify tag exists
    let tags = repo
        .get_all()
        .await
        .map_err(|e| TagError::Internal(format!("Failed to fetch tags: {}", e)))?;

    if !tags.iter().any(|(tag_id, _)| *tag_id == id as i64) {
        return Err(TagError::NotFound);
    }

    repo.update(id as i64, &body.label)
        .await
        .map_err(|e| TagError::Internal(format!("Failed to update tag: {}", e)))?;

    tracing::info!("Updated tag: id={}, label={}", id, body.label);

    Ok(Json(TagResource {
        id,
        label: body.label,
    }))
}

/// DELETE /api/v5/tag/{id} - Delete a tag
pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, TagError> {
    let repo = TagRepository::new(state.db.clone());

    repo.delete(id as i64)
        .await
        .map_err(|e| TagError::Internal(format!("Failed to delete tag: {}", e)))?;

    tracing::info!("Deleted tag: id={}", id);

    Ok(Json(serde_json::json!({})))
}

/// Parse a JSON-serialized tags string into a `Vec<i64>`.
fn parse_tags(tags_json: &str) -> Vec<i64> {
    serde_json::from_str::<Vec<i64>>(tags_json).unwrap_or_default()
}

/// GET /api/v5/tag/detail - Get all tags with usage details
pub async fn get_tag_details(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TagDetailsResource>>, TagError> {
    let tag_repo = TagRepository::new(state.db.clone());
    let tags = tag_repo
        .get_all()
        .await
        .map_err(|e| TagError::Internal(format!("Failed to fetch tags: {}", e)))?;

    // Load all entities that carry tags
    let delay_profiles = DelayProfileRepository::new(state.db.clone())
        .get_all()
        .await
        .unwrap_or_default();
    let notifications = NotificationRepository::new(state.db.clone())
        .get_all()
        .await
        .unwrap_or_default();
    let indexers = IndexerRepository::new(state.db.clone())
        .get_all()
        .await
        .unwrap_or_default();
    let download_clients = DownloadClientRepository::new(state.db.clone())
        .get_all()
        .await
        .unwrap_or_default();
    // Note: series tags are not stored in the DB schema yet, so series_ids is always empty

    let resources: Vec<TagDetailsResource> = tags
        .into_iter()
        .map(|(id, label)| {
            let tag_id = id;

            let delay_profile_ids: Vec<i32> = delay_profiles
                .iter()
                .filter(|dp| parse_tags(&dp.tags).contains(&tag_id))
                .map(|dp| dp.id as i32)
                .collect();
            let notification_ids: Vec<i32> = notifications
                .iter()
                .filter(|n| parse_tags(&n.tags).contains(&tag_id))
                .map(|n| n.id as i32)
                .collect();
            let indexer_ids: Vec<i32> = indexers
                .iter()
                .filter(|i| parse_tags(&i.tags).contains(&tag_id))
                .map(|i| i.id as i32)
                .collect();
            let download_client_ids: Vec<i32> = download_clients
                .iter()
                .filter(|dc| parse_tags(&dc.tags).contains(&tag_id))
                .map(|dc| dc.id as i32)
                .collect();
            let series_ids: Vec<i32> = vec![];

            TagDetailsResource {
                id: id as i32,
                label,
                delay_profile_ids,
                import_list_ids: vec![],
                notification_ids,
                restriction_ids: vec![],
                indexer_ids,
                download_client_ids,
                auto_tag_ids: vec![],
                series_ids,
            }
        })
        .collect();

    Ok(Json(resources))
}

/// Error type for tag operations
#[derive(Debug)]
pub enum TagError {
    NotFound,
    Validation(String),
    Internal(String),
}

impl IntoResponse for TagError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            TagError::NotFound => (StatusCode::NOT_FOUND, "Tag not found".to_string()),
            TagError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            TagError::Internal(msg) => {
                tracing::error!("Tag error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_tags).post(create_tag))
        .route("/{id}", get(get_tag).put(update_tag).delete(delete_tag))
        .route("/detail", get(get_tag_details))
}
