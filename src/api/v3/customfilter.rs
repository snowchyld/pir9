//! Custom Filter API endpoints

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
pub struct CustomFilterResource {
    pub id: i32,
    #[serde(rename = "type")]
    pub filter_type: String,
    pub label: String,
    pub filters: Vec<FilterResource>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterResource {
    pub key: String,
    pub value: Vec<serde_json::Value>,
    #[serde(rename = "type")]
    pub filter_type: Option<String>,
}

/// GET /api/v3/customfilter
pub async fn get_custom_filters() -> Json<Vec<CustomFilterResource>> {
    Json(vec![])
}

/// GET /api/v3/customfilter/:id
pub async fn get_custom_filter(Path(id): Path<i32>) -> Json<Option<CustomFilterResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/customfilter
pub async fn create_custom_filter(
    Json(body): Json<CustomFilterResource>,
) -> Json<CustomFilterResource> {
    Json(body)
}

/// PUT /api/v3/customfilter/:id
pub async fn update_custom_filter(
    Path(id): Path<i32>,
    Json(mut body): Json<CustomFilterResource>,
) -> Json<CustomFilterResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/customfilter/:id
pub async fn delete_custom_filter(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_custom_filters).post(create_custom_filter))
        .route(
            "/{id}",
            get(get_custom_filter)
                .put(update_custom_filter)
                .delete(delete_custom_filter),
        )
}
