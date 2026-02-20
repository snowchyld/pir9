//! Custom Format API endpoints (v5)

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
pub struct CustomFormatResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub include_custom_format_when_renaming: bool,
    pub specifications: Vec<CustomFormatSpecificationResource>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormatSpecificationResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub implementation: String,
    pub implementation_name: String,
    pub negate: bool,
    pub required: bool,
    pub fields: Vec<serde_json::Value>,
}

pub async fn get_custom_formats() -> Json<Vec<CustomFormatResource>> {
    Json(vec![])
}

pub async fn get_custom_format(Path(id): Path<i32>) -> Json<Option<CustomFormatResource>> {
    let _ = id;
    Json(None)
}

pub async fn create_custom_format(Json(body): Json<CustomFormatResource>) -> Json<CustomFormatResource> {
    Json(body)
}

pub async fn update_custom_format(Path(id): Path<i32>, Json(mut body): Json<CustomFormatResource>) -> Json<CustomFormatResource> {
    body.id = id;
    Json(body)
}

pub async fn delete_custom_format(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

pub async fn get_custom_format_schema() -> Json<Vec<CustomFormatSpecificationResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_custom_formats).post(create_custom_format))
        .route("/{id}", get(get_custom_format).put(update_custom_format).delete(delete_custom_format))
        .route("/schema", get(get_custom_format_schema))
}
