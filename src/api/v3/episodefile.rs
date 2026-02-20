//! Episode File API endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::EpisodeFileRepository;
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFileQuery {
    pub series_id: Option<i64>,
    pub episode_file_ids: Option<String>, // comma-separated IDs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFileResource {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: String,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub languages: Vec<LanguageResource>,
    pub quality: QualityModel,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
    pub indexer_flags: Option<i32>,
    pub release_type: String,
    pub media_info: Option<MediaInfoResource>,
    pub quality_cutoff_not_met: bool,
}

impl EpisodeFileResource {
    fn from_db_model(ef: &crate::core::datastore::models::EpisodeFileDbModel) -> Self {
        // Parse quality JSON
        let quality: QualityModel = serde_json::from_str(&ef.quality).unwrap_or_else(|_| {
            QualityModel {
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
            }
        });

        // Parse languages JSON
        let languages: Vec<LanguageResource> = serde_json::from_str(&ef.languages).unwrap_or_else(|_| {
            vec![LanguageResource {
                id: 1,
                name: "English".to_string(),
            }]
        });

        Self {
            id: ef.id,
            series_id: ef.series_id,
            season_number: ef.season_number,
            relative_path: ef.relative_path.clone(),
            path: ef.path.clone(),
            size: ef.size,
            date_added: ef.date_added.to_rfc3339(),
            scene_name: ef.scene_name.clone(),
            release_group: ef.release_group.clone(),
            languages,
            quality,
            custom_formats: vec![],
            custom_format_score: 0,
            indexer_flags: None,
            release_type: "unknown".to_string(),
            media_info: None,
            quality_cutoff_not_met: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfoResource {
    pub id: i32,
    pub audio_bitrate: i32,
    pub audio_channels: f64,
    pub audio_codec: Option<String>,
    pub audio_languages: Option<String>,
    pub audio_stream_count: i32,
    pub video_bit_depth: i32,
    pub video_bitrate: i32,
    pub video_codec: Option<String>,
    pub video_fps: f64,
    pub video_dynamic_range: Option<String>,
    pub video_dynamic_range_type: Option<String>,
    pub resolution: String,
    pub run_time: String,
    pub scan_type: String,
    pub subtitles: Option<String>,
}

/// GET /api/v3/episodefile
pub async fn get_episode_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EpisodeFileQuery>,
) -> Result<Json<Vec<EpisodeFileResource>>, EpisodeFileError> {
    let repo = EpisodeFileRepository::new(state.db.clone());

    let files = if let Some(series_id) = query.series_id {
        repo.get_by_series_id(series_id).await
            .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode files: {}", e)))?
    } else if let Some(ids_str) = query.episode_file_ids {
        let ids: Vec<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        let mut files = Vec::new();
        for id in ids {
            if let Some(ef) = repo.get_by_id(id).await
                .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode file: {}", e)))? {
                files.push(ef);
            }
        }
        files
    } else {
        repo.get_all().await
            .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode files: {}", e)))?
    };

    let resources: Vec<EpisodeFileResource> = files
        .iter()
        .map(EpisodeFileResource::from_db_model)
        .collect();

    Ok(Json(resources))
}

/// GET /api/v3/episodefile/:id
pub async fn get_episode_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<EpisodeFileResource>, EpisodeFileError> {
    let repo = EpisodeFileRepository::new(state.db.clone());

    let file = repo.get_by_id(id).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode file: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    Ok(Json(EpisodeFileResource::from_db_model(&file)))
}

/// DELETE /api/v3/episodefile/:id
pub async fn delete_episode_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, EpisodeFileError> {
    let repo = EpisodeFileRepository::new(state.db.clone());

    repo.delete(id).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to delete episode file: {}", e)))?;

    tracing::info!("Deleted episode file: id={}", id);
    Ok(Json(serde_json::json!({})))
}

/// PUT /api/v3/episodefile/:id
pub async fn update_episode_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<EpisodeFileResource>,
) -> Result<Json<EpisodeFileResource>, EpisodeFileError> {
    let repo = EpisodeFileRepository::new(state.db.clone());

    let mut file = repo.get_by_id(id).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode file: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    // Update fields
    file.release_group = body.release_group.clone();
    file.scene_name = body.scene_name.clone();
    file.quality = serde_json::to_string(&body.quality).unwrap_or_default();
    file.languages = serde_json::to_string(&body.languages).unwrap_or_default();

    repo.update(&file).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to update episode file: {}", e)))?;

    Ok(Json(EpisodeFileResource::from_db_model(&file)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkDeleteRequest {
    pub episode_file_ids: Vec<i64>,
}

/// DELETE /api/v3/episodefile/bulk
pub async fn delete_episode_files_bulk(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeleteRequest>,
) -> Result<Json<serde_json::Value>, EpisodeFileError> {
    let repo = EpisodeFileRepository::new(state.db.clone());

    for id in &body.episode_file_ids {
        if let Err(e) = repo.delete(*id).await {
            tracing::warn!("Failed to delete episode file {}: {}", id, e);
        }
    }

    Ok(Json(serde_json::json!({})))
}

/// PUT /api/v3/episodefile/editor
pub async fn update_episode_files_editor(
    Json(_body): Json<serde_json::Value>,
) -> Json<Vec<EpisodeFileResource>> {
    Json(vec![])
}

/// Error type for episode file operations
#[derive(Debug)]
pub enum EpisodeFileError {
    NotFound,
    Internal(String),
}

impl IntoResponse for EpisodeFileError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            EpisodeFileError::NotFound => (StatusCode::NOT_FOUND, "Episode file not found".to_string()),
            EpisodeFileError::Internal(msg) => {
                tracing::error!("Episode file error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_episode_files))
        .route(
            "/{id}",
            get(get_episode_file)
                .delete(delete_episode_file)
                .put(update_episode_file),
        )
        .route("/bulk", delete(delete_episode_files_bulk))
        .route("/editor", put(update_episode_files_editor))
}
