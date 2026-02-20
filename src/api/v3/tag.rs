//! Tag API endpoints

use axum::{
    extract::{Path, State},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{
    DelayProfileRepository, DownloadClientRepository, IndexerRepository,
    NotificationRepository, TagRepository,
};
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TagResource {
    #[serde(default)]
    pub id: i32,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TagDetailsResource {
    pub id: i32,
    pub label: String,
    #[serde(default)]
    pub series_ids: Vec<i32>,
    #[serde(default)]
    pub notification_ids: Vec<i32>,
    #[serde(default)]
    pub restriction_ids: Vec<i32>,
    #[serde(default)]
    pub indexer_ids: Vec<i32>,
    #[serde(default)]
    pub download_client_ids: Vec<i32>,
    #[serde(default)]
    pub auto_tag_ids: Vec<i32>,
    #[serde(default)]
    pub import_list_ids: Vec<i32>,
}

/// GET /api/v3/tag
pub async fn get_tags(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<TagResource>> {
    let repo = TagRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(tags) => {
            let resources: Vec<TagResource> = tags
                .into_iter()
                .map(|(id, label)| TagResource { id: id as i32, label })
                .collect();
            Json(resources)
        }
        Err(e) => {
            tracing::error!("Failed to fetch tags: {}", e);
            Json(vec![])
        }
    }
}

/// GET /api/v3/tag/:id
pub async fn get_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Json<Option<TagResource>> {
    let repo = TagRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(tags) => {
            let tag = tags
                .into_iter()
                .find(|(tag_id, _)| *tag_id == id as i64)
                .map(|(id, label)| TagResource { id: id as i32, label });
            Json(tag)
        }
        Err(e) => {
            tracing::error!("Failed to fetch tag {}: {}", id, e);
            Json(None)
        }
    }
}

/// POST /api/v3/tag
pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TagResource>,
) -> Json<TagResource> {
    let repo = TagRepository::new(state.db.clone());

    match repo.insert(&body.label).await {
        Ok(id) => Json(TagResource {
            id: id as i32,
            label: body.label,
        }),
        Err(e) => {
            tracing::error!("Failed to create tag: {}", e);
            Json(TagResource {
                id: 0,
                label: body.label,
            })
        }
    }
}

/// PUT /api/v3/tag/:id
pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<TagResource>,
) -> Json<TagResource> {
    let repo = TagRepository::new(state.db.clone());

    match repo.update(id as i64, &body.label).await {
        Ok(_) => Json(TagResource { id, label: body.label }),
        Err(e) => {
            tracing::error!("Failed to update tag {}: {}", id, e);
            Json(TagResource { id, label: body.label })
        }
    }
}

/// DELETE /api/v3/tag/:id
pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Json<serde_json::Value> {
    let repo = TagRepository::new(state.db.clone());

    if let Err(e) = repo.delete(id as i64).await {
        tracing::error!("Failed to delete tag {}: {}", id, e);
    }
    Json(serde_json::json!({}))
}

/// Parse a JSON-serialized tags string into a Vec<i64>
fn parse_tags(tags_json: &str) -> Vec<i64> {
    serde_json::from_str::<Vec<i64>>(tags_json).unwrap_or_default()
}

/// GET /api/v3/tag/detail
pub async fn get_tag_details(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<TagDetailsResource>> {
    let tag_repo = TagRepository::new(state.db.clone());
    let tags = match tag_repo.get_all().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to fetch tags: {}", e);
            return Json(vec![]);
        }
    };

    let delay_profiles = DelayProfileRepository::new(state.db.clone()).get_all().await.unwrap_or_default();
    let notifications = NotificationRepository::new(state.db.clone()).get_all().await.unwrap_or_default();
    let indexers = IndexerRepository::new(state.db.clone()).get_all().await.unwrap_or_default();
    let download_clients = DownloadClientRepository::new(state.db.clone()).get_all().await.unwrap_or_default();

    let resources: Vec<TagDetailsResource> = tags
        .into_iter()
        .map(|(id, label)| {
            let tag_id = id;

            let notification_ids: Vec<i32> = notifications.iter()
                .filter(|n| parse_tags(&n.tags).contains(&tag_id))
                .map(|n| n.id as i32)
                .collect();
            let indexer_ids: Vec<i32> = indexers.iter()
                .filter(|i| parse_tags(&i.tags).contains(&tag_id))
                .map(|i| i.id as i32)
                .collect();
            let download_client_ids: Vec<i32> = download_clients.iter()
                .filter(|dc| parse_tags(&dc.tags).contains(&tag_id))
                .map(|dc| dc.id as i32)
                .collect();

            TagDetailsResource {
                id: id as i32,
                label,
                series_ids: vec![],
                notification_ids,
                restriction_ids: vec![],
                indexer_ids,
                download_client_ids,
                auto_tag_ids: vec![],
                import_list_ids: delay_profiles.iter()
                    .filter(|dp| parse_tags(&dp.tags).contains(&tag_id))
                    .map(|dp| dp.id as i32)
                    .collect(),
            }
        })
        .collect();

    Json(resources)
}

/// GET /api/v3/tag/detail/:id
pub async fn get_tag_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Json<Option<TagDetailsResource>> {
    let tag_repo = TagRepository::new(state.db.clone());
    let tags = match tag_repo.get_all().await {
        Ok(t) => t,
        Err(_) => return Json(None),
    };

    let tag = match tags.into_iter().find(|(tag_id, _)| *tag_id == id as i64) {
        Some(t) => t,
        None => return Json(None),
    };

    let delay_profiles = DelayProfileRepository::new(state.db.clone()).get_all().await.unwrap_or_default();
    let notifications = NotificationRepository::new(state.db.clone()).get_all().await.unwrap_or_default();
    let indexers = IndexerRepository::new(state.db.clone()).get_all().await.unwrap_or_default();
    let download_clients = DownloadClientRepository::new(state.db.clone()).get_all().await.unwrap_or_default();

    let tag_id = tag.0;
    Json(Some(TagDetailsResource {
        id: tag.0 as i32,
        label: tag.1,
        series_ids: vec![],
        notification_ids: notifications.iter()
            .filter(|n| parse_tags(&n.tags).contains(&tag_id))
            .map(|n| n.id as i32)
            .collect(),
        restriction_ids: vec![],
        indexer_ids: indexers.iter()
            .filter(|i| parse_tags(&i.tags).contains(&tag_id))
            .map(|i| i.id as i32)
            .collect(),
        download_client_ids: download_clients.iter()
            .filter(|dc| parse_tags(&dc.tags).contains(&tag_id))
            .map(|dc| dc.id as i32)
            .collect(),
        auto_tag_ids: vec![],
        import_list_ids: delay_profiles.iter()
            .filter(|dp| parse_tags(&dp.tags).contains(&tag_id))
            .map(|dp| dp.id as i32)
            .collect(),
    }))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_tags).post(create_tag))
        .route("/{id}", get(get_tag).put(update_tag).delete(delete_tag))
        .route("/detail", get(get_tag_details))
        .route("/detail/{id}", get(get_tag_detail))
}
