//! Manual Import API endpoints (v5)
//!
//! Scans a folder for video files, parses filenames, and attempts to match
//! them to series/episodes in the database.

use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::SeriesRepository;
use crate::core::parser;
use crate::web::AppState;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "avi", "mp4", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ManualImportQuery {
    pub folder: Option<String>,
    pub download_id: Option<String>,
    pub series_id: Option<i64>,
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

/// GET /api/v5/manualimport — scan a folder for importable video files
pub async fn get_manual_import(
    State(state): State<Arc<AppState>>,
    query: Query<ManualImportQuery>,
) -> Json<Vec<ManualImportResource>> {
    let folder = match &query.folder {
        Some(f) => f.clone(),
        None => return Json(vec![]),
    };

    let folder_path = std::path::Path::new(&folder);
    if !folder_path.is_dir() {
        return Json(vec![]);
    }

    // Collect video files from the folder
    let mut files = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(folder_path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                        files.push(path);
                    }
                }
            }
        }
    }

    let series_repo = SeriesRepository::new(state.db.clone());
    let all_series = series_repo.get_all().await.unwrap_or_default();

    let mut results = Vec::new();
    for (idx, file_path) in files.iter().enumerate() {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let relative_path = file_path
            .strip_prefix(folder_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let size = tokio::fs::metadata(file_path)
            .await
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let parsed = parser::parse_title(&file_name);
        let quality = parsed
            .as_ref()
            .map(|p| serde_json::to_value(&p.quality).unwrap_or_default())
            .unwrap_or(serde_json::json!({}));

        // Try to match to a series (year-aware scoring picks the best candidate)
        let matched_series = parsed.as_ref().and_then(|info| {
            parser::best_series_match(info, &all_series)
                .map(|idx| &all_series[idx])
        });

        let series_json = matched_series.map(|s| {
            serde_json::json!({
                "id": s.id,
                "title": s.title,
                "tvdbId": s.tvdb_id,
            })
        });

        let season_number = parsed.as_ref().and_then(|p| p.season_number);

        results.push(ManualImportResource {
            id: (idx + 1) as i32,
            path: file_path.to_string_lossy().to_string(),
            relative_path,
            folder_name: folder_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            name: file_name,
            size,
            series: series_json,
            season_number,
            episodes: vec![],
            quality,
            languages: vec![],
            rejections: vec![],
        });
    }

    Json(results)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_manual_import))
}
