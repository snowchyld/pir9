//! Episode File API endpoints (v5)

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
    pub episode_file_ids: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub media_info: Option<serde_json::Value>,
    pub quality_cutoff_not_met: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
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

        // Parse media info JSON
        let media_info: Option<serde_json::Value> = ef
            .media_info
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

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
            media_info,
            quality_cutoff_not_met: false,
        }
    }
}

/// GET /api/v5/episodefile - List episode files
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

/// GET /api/v5/episodefile/{id} - Get a specific episode file
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

/// DELETE /api/v5/episodefile/{id} - Delete an episode file
pub async fn delete_episode_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, EpisodeFileError> {
    use crate::core::datastore::repositories::EpisodeRepository;

    let file_repo = EpisodeFileRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());

    // Get the file info before deleting
    let file = file_repo.get_by_id(id).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode file: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    let file_path = std::path::Path::new(&file.path);

    // Delete the actual file from disk
    if file_path.exists() {
        match tokio::fs::remove_file(&file.path).await {
            Ok(_) => {
                tracing::info!("Deleted file from disk: {}", file.path);
                crate::core::logging::log_info(
                    "EpisodeFileDeleted",
                    &format!("Deleted episode file: {}", file.relative_path)
                ).await;
            }
            Err(e) => {
                tracing::error!("Failed to delete file from disk: {} - {}", file.path, e);
                // Continue to delete from DB even if disk delete fails
                // This matches Sonarr behavior where the DB record can be removed
                // even if the file is locked or inaccessible
                crate::core::logging::log_warn(
                    "EpisodeFileDeleted",
                    &format!("Removed episode file record, but failed to delete from disk: {} ({})", file.relative_path, e)
                ).await;
            }
        }
    } else {
        tracing::debug!("File already missing from disk: {}", file.path);
    }

    // Update any episodes that reference this file to clear the reference
    let episodes = episode_repo.get_by_series_id(file.series_id).await
        .unwrap_or_default();

    for mut ep in episodes {
        if ep.episode_file_id == Some(id) {
            ep.episode_file_id = None;
            ep.has_file = false;
            if let Err(e) = episode_repo.update(&ep).await {
                tracing::warn!("Failed to update episode {} after file deletion: {}", ep.id, e);
            }
        }
    }

    // Delete the database record
    file_repo.delete(id).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to delete episode file record: {}", e)))?;

    tracing::info!("Deleted episode file: id={}, path={}", id, file.path);

    Ok(Json(serde_json::json!({})))
}

/// PUT /api/v5/episodefile/{id} - Update an episode file
pub async fn update_episode_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<EpisodeFileResource>,
) -> Result<Json<EpisodeFileResource>, EpisodeFileError> {
    let repo = EpisodeFileRepository::new(state.db.clone());

    let mut file = repo.get_by_id(id).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode file: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    // Update fields that can be modified
    file.release_group = body.release_group.clone();
    file.scene_name = body.scene_name.clone();
    file.quality = serde_json::to_string(&body.quality).unwrap_or_default();
    file.languages = serde_json::to_string(&body.languages).unwrap_or_default();

    repo.update(&file).await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to update episode file: {}", e)))?;

    tracing::info!("Updated episode file: id={}", id);

    Ok(Json(EpisodeFileResource::from_db_model(&file)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkDeleteRequest {
    pub episode_file_ids: Vec<i64>,
}

/// DELETE /api/v5/episodefile/bulk - Delete multiple episode files
pub async fn delete_episode_files_bulk(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeleteRequest>,
) -> Result<Json<serde_json::Value>, EpisodeFileError> {
    use crate::core::datastore::repositories::EpisodeRepository;

    let file_repo = EpisodeFileRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());

    let mut deleted_count = 0;
    let mut disk_errors = 0;

    for id in &body.episode_file_ids {
        // Get the file info before deleting
        let file = match file_repo.get_by_id(*id).await {
            Ok(Some(f)) => f,
            Ok(None) => {
                tracing::warn!("Episode file {} not found during bulk delete", id);
                continue;
            }
            Err(e) => {
                tracing::warn!("Failed to fetch episode file {}: {}", id, e);
                continue;
            }
        };

        let file_path = std::path::Path::new(&file.path);

        // Delete the actual file from disk
        if file_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&file.path).await {
                tracing::warn!("Failed to delete file from disk: {} - {}", file.path, e);
                disk_errors += 1;
            }
        }

        // Update any episodes that reference this file
        if let Ok(episodes) = episode_repo.get_by_series_id(file.series_id).await {
            for mut ep in episodes {
                if ep.episode_file_id == Some(*id) {
                    ep.episode_file_id = None;
                    ep.has_file = false;
                    let _ = episode_repo.update(&ep).await;
                }
            }
        }

        // Delete the database record
        if let Err(e) = file_repo.delete(*id).await {
            tracing::warn!("Failed to delete episode file record {}: {}", id, e);
        } else {
            deleted_count += 1;
        }
    }

    tracing::info!(
        "Bulk deleted {} episode files ({} disk errors)",
        deleted_count, disk_errors
    );

    if deleted_count > 0 {
        crate::core::logging::log_info(
            "EpisodeFilesDeleted",
            &format!("Bulk deleted {} episode files", deleted_count)
        ).await;
    }

    Ok(Json(serde_json::json!({})))
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
        .route("/{id}", get(get_episode_file).delete(delete_episode_file).put(update_episode_file))
        .route("/bulk", delete(delete_episode_files_bulk))
}
