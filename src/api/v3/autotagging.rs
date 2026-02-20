//! Auto Tagging API endpoints

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
pub struct AutoTaggingResource {
    pub id: i32,
    pub name: String,
    pub remove_tags_automatically: bool,
    pub tags: Vec<i32>,
    pub specifications: Vec<AutoTaggingSpecificationResource>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTaggingSpecificationResource {
    pub id: i32,
    pub name: String,
    pub implementation: String,
    pub implementation_name: String,
    pub negate: bool,
    pub required: bool,
    pub fields: Vec<FieldResource>,
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

/// GET /api/v3/autotagging
pub async fn get_auto_tagging() -> Json<Vec<AutoTaggingResource>> {
    Json(vec![])
}

/// GET /api/v3/autotagging/:id
pub async fn get_auto_tagging_by_id(Path(id): Path<i32>) -> Json<Option<AutoTaggingResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/autotagging
pub async fn create_auto_tagging(
    Json(body): Json<AutoTaggingResource>,
) -> Json<AutoTaggingResource> {
    Json(body)
}

/// PUT /api/v3/autotagging/:id
pub async fn update_auto_tagging(
    Path(id): Path<i32>,
    Json(mut body): Json<AutoTaggingResource>,
) -> Json<AutoTaggingResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/autotagging/:id
pub async fn delete_auto_tagging(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// GET /api/v3/autotagging/schema
pub async fn get_auto_tagging_schema() -> Json<Vec<AutoTaggingSpecificationResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_auto_tagging).post(create_auto_tagging))
        .route(
            "/{id}",
            get(get_auto_tagging_by_id)
                .put(update_auto_tagging)
                .delete(delete_auto_tagging),
        )
        .route("/schema", get(get_auto_tagging_schema))
}
