//! Release Profile API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseProfileResource {
    pub id: i32,
    pub name: Option<String>,
    pub enabled: bool,
    pub required: Vec<String>,
    pub ignored: Vec<String>,
    pub indexer_id: i32,
    pub tags: Vec<i32>,
}

/// GET /api/v3/releaseprofile
pub async fn get_release_profiles() -> Json<Vec<ReleaseProfileResource>> {
    Json(vec![])
}

/// GET /api/v3/releaseprofile/:id
pub async fn get_release_profile(Path(id): Path<i32>) -> Json<Option<ReleaseProfileResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/releaseprofile
pub async fn create_release_profile(
    Json(body): Json<ReleaseProfileResource>,
) -> Json<ReleaseProfileResource> {
    Json(body)
}

/// PUT /api/v3/releaseprofile/:id
pub async fn update_release_profile(
    Path(id): Path<i32>,
    Json(mut body): Json<ReleaseProfileResource>,
) -> Json<ReleaseProfileResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/releaseprofile/:id
pub async fn delete_release_profile(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_release_profiles).post(create_release_profile))
        .route(
            "/{id}",
            get(get_release_profile)
                .put(update_release_profile)
                .delete(delete_release_profile),
        )
}
