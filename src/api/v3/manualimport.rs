//! Manual Import API endpoints

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

#[derive(Debug, Serialize, Deserialize)]
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
    pub episode_file_id: Option<i32>,
    pub release_group: Option<String>,
    pub quality: QualityModel,
    pub languages: Vec<LanguageResource>,
    pub quality_weight: i32,
    pub download_id: Option<String>,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
    pub indexer_flags: i32,
    pub release_type: String,
    pub rejections: Vec<RejectionResource>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectionResource {
    pub reason: String,
    #[serde(rename = "type")]
    pub rejection_type: String,
}

/// GET /api/v3/manualimport
pub async fn get_manual_import(_query: Query<ManualImportQuery>) -> Json<Vec<ManualImportResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_manual_import))
}
