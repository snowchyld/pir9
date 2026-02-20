//! Remote Path Mapping API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePathMappingResource {
    pub id: i32,
    pub host: String,
    pub remote_path: String,
    pub local_path: String,
}

/// GET /api/v3/remotepathmapping
pub async fn get_remote_path_mappings() -> Json<Vec<RemotePathMappingResource>> {
    Json(vec![])
}

/// GET /api/v3/remotepathmapping/:id
pub async fn get_remote_path_mapping(Path(id): Path<i32>) -> Json<Option<RemotePathMappingResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/remotepathmapping
pub async fn create_remote_path_mapping(
    Json(body): Json<RemotePathMappingResource>,
) -> Json<RemotePathMappingResource> {
    Json(body)
}

/// PUT /api/v3/remotepathmapping/:id
pub async fn update_remote_path_mapping(
    Path(id): Path<i32>,
    Json(mut body): Json<RemotePathMappingResource>,
) -> Json<RemotePathMappingResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/remotepathmapping/:id
pub async fn delete_remote_path_mapping(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_remote_path_mappings).post(create_remote_path_mapping))
        .route(
            "/{id}",
            get(get_remote_path_mapping)
                .put(update_remote_path_mapping)
                .delete(delete_remote_path_mapping),
        )
}
