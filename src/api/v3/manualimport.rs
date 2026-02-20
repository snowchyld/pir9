//! Manual Import API endpoints

use axum::{extract::{Query, State}, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
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

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "avi", "mp4", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg",
];

/// GET /api/v3/manualimport
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

    let series_repo = crate::core::datastore::repositories::SeriesRepository::new(state.db.clone());
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

        let parsed = crate::core::parser::parse_title(&file_name);
        let quality_model = parsed
            .as_ref()
            .map(|p| QualityModel {
                quality: QualityResource {
                    id: p.quality.quality.weight(),
                    name: format!("{:?}", p.quality.quality),
                    source: "unknown".to_string(),
                    resolution: p.quality.quality.resolution_width(),
                },
                revision: RevisionResource {
                    version: p.quality.revision.version,
                    real: p.quality.revision.real,
                    is_repack: p.quality.revision.is_repack,
                },
            })
            .unwrap_or(QualityModel {
                quality: QualityResource {
                    id: 0,
                    name: "Unknown".to_string(),
                    source: "unknown".to_string(),
                    resolution: 0,
                },
                revision: RevisionResource {
                    version: 1,
                    real: 0,
                    is_repack: false,
                },
            });

        let matched_series = parsed.as_ref().and_then(|info| {
            all_series
                .iter()
                .find(|s| crate::core::parser::title_matches_series(info, &s.title))
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
            episode_file_id: None,
            release_group: parsed.as_ref().and_then(|p| p.release_group.clone()),
            quality: quality_model,
            languages: vec![],
            quality_weight: 0,
            download_id: query.download_id.clone(),
            custom_formats: vec![],
            custom_format_score: 0,
            indexer_flags: 0,
            release_type: "unknown".to_string(),
            rejections: vec![],
        });
    }

    Json(results)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_manual_import))
}
