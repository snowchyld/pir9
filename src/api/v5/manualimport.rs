//! Manual Import API endpoints (v5)

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportQuery {
    pub folder: Option<String>,
    pub download_id: Option<String>,
    pub series_id: Option<i32>,
    pub season_number: Option<i32>,
    pub filter_existing_files: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportResource {
    pub id: i32,
    pub path: String,
    pub relative_path: String,
    pub folder_name: String,
    pub name: String,
    pub size: i64,
    pub series: Option<serde_json::Value>,
    pub season_number: Option<i32>,
    pub episodes: Vec<serde_json::Value>,
    pub quality: serde_json::Value,
    pub languages: Vec<serde_json::Value>,
    pub rejections: Vec<serde_json::Value>,
}

pub async fn get_manual_import(_query: Query<ManualImportQuery>) -> Json<Vec<ManualImportResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_manual_import))
}
