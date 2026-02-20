//! Series Editor API endpoints

use axum::{response::Json, routing::put, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesEditorResource {
    pub series_ids: Vec<i32>,
    pub monitored: Option<bool>,
    pub quality_profile_id: Option<i32>,
    pub series_type: Option<String>,
    pub season_folder: Option<bool>,
    pub root_folder_path: Option<String>,
    pub tags: Option<Vec<i32>>,
    pub apply_tags: Option<String>,
    pub move_files: Option<bool>,
    pub delete_files: Option<bool>,
    pub add_import_list_exclusion: Option<bool>,
}

/// PUT /api/v3/series/editor
pub async fn update_series_editor(
    Json(_body): Json<SeriesEditorResource>,
) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

/// DELETE /api/v3/series/editor
pub async fn delete_series_editor(
    Json(_body): Json<SeriesEditorResource>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", put(update_series_editor).delete(delete_series_editor))
}
