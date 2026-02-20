//! Tag API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
pub async fn get_tags() -> Json<Vec<TagResource>> {
    Json(vec![])
}

/// GET /api/v3/tag/:id
pub async fn get_tag(Path(id): Path<i32>) -> Json<Option<TagResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/tag
pub async fn create_tag(Json(mut body): Json<TagResource>) -> Json<TagResource> {
    body.id = 1; // Mock ID
    Json(body)
}

/// PUT /api/v3/tag/:id
pub async fn update_tag(
    Path(id): Path<i32>,
    Json(mut body): Json<TagResource>,
) -> Json<TagResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/tag/:id
pub async fn delete_tag(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// GET /api/v3/tag/detail
pub async fn get_tag_details() -> Json<Vec<TagDetailsResource>> {
    Json(vec![])
}

/// GET /api/v3/tag/detail/:id
pub async fn get_tag_detail(Path(id): Path<i32>) -> Json<Option<TagDetailsResource>> {
    let _ = id;
    Json(None)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_tags).post(create_tag))
        .route("/{id}", get(get_tag).put(update_tag).delete(delete_tag))
        .route("/detail", get(get_tag_details))
        .route("/detail/{id}", get(get_tag_detail))
}
