//! Metadata API endpoints

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
pub struct MetadataResource {
    pub id: i32,
    pub name: String,
    pub fields: Vec<FieldResource>,
    pub implementation_name: String,
    pub implementation: String,
    pub config_contract: String,
    pub info_link: Option<String>,
    pub message: Option<ProviderMessage>,
    pub tags: Vec<i32>,
    pub presets: Vec<serde_json::Value>,
    pub enable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldResource {
    pub order: i32,
    pub name: String,
    pub label: String,
    pub unit: Option<String>,
    pub help_text: Option<String>,
    pub help_text_warning: Option<String>,
    pub help_link: Option<String>,
    pub value: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub field_type: String,
    pub advanced: bool,
    pub select_options: Option<Vec<serde_json::Value>>,
    pub select_options_provider_action: Option<String>,
    pub section: Option<String>,
    pub hidden: Option<String>,
    pub privacy: Option<String>,
    pub placeholder: Option<String>,
    pub is_float: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub message_type: String,
}

/// GET /api/v3/metadata
pub async fn get_metadata() -> Json<Vec<MetadataResource>> {
    Json(vec![])
}

/// GET /api/v3/metadata/:id
pub async fn get_metadata_by_id(Path(id): Path<i32>) -> Json<Option<MetadataResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/metadata
pub async fn create_metadata(Json(body): Json<MetadataResource>) -> Json<MetadataResource> {
    Json(body)
}

/// PUT /api/v3/metadata/:id
pub async fn update_metadata(
    Path(id): Path<i32>,
    Json(mut body): Json<MetadataResource>,
) -> Json<MetadataResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/metadata/:id
pub async fn delete_metadata(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// POST /api/v3/metadata/test
pub async fn test_metadata(Json(_body): Json<MetadataResource>) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

/// POST /api/v3/metadata/testall
pub async fn test_all_metadata() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

/// GET /api/v3/metadata/schema
pub async fn get_metadata_schema() -> Json<Vec<MetadataResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_metadata).post(create_metadata))
        .route(
            "/{id}",
            get(get_metadata_by_id)
                .put(update_metadata)
                .delete(delete_metadata),
        )
        .route("/test", post(test_metadata))
        .route("/testall", post(test_all_metadata))
        .route("/schema", get(get_metadata_schema))
}
