//! Episode File API endpoints (v5)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{
    EpisodeFileRepository, EpisodeRepository, SeriesRepository,
};
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
        let quality: QualityModel =
            serde_json::from_str(&ef.quality).unwrap_or_else(|_| QualityModel {
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

        // Parse languages JSON
        let languages: Vec<LanguageResource> =
            serde_json::from_str(&ef.languages).unwrap_or_else(|_| {
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
        repo.get_by_series_id(series_id).await.map_err(|e| {
            EpisodeFileError::Internal(format!("Failed to fetch episode files: {}", e))
        })?
    } else if let Some(ids_str) = query.episode_file_ids {
        let ids: Vec<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        repo.get_by_ids(&ids).await.map_err(|e| {
            EpisodeFileError::Internal(format!("Failed to fetch episode files: {}", e))
        })?
    } else {
        repo.get_all().await.map_err(|e| {
            EpisodeFileError::Internal(format!("Failed to fetch episode files: {}", e))
        })?
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

    let file = repo
        .get_by_id(id)
        .await
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
    let file = file_repo
        .get_by_id(id)
        .await
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
                    &format!("Deleted episode file: {}", file.relative_path),
                )
                .await;
            }
            Err(e) => {
                tracing::error!("Failed to delete file from disk: {} - {}", file.path, e);
                // Continue to delete from DB even if disk delete fails
                // This matches Sonarr behavior where the DB record can be removed
                // even if the file is locked or inaccessible
                crate::core::logging::log_warn(
                    "EpisodeFileDeleted",
                    &format!(
                        "Removed episode file record, but failed to delete from disk: {} ({})",
                        file.relative_path, e
                    ),
                )
                .await;
            }
        }
    } else {
        tracing::debug!("File already missing from disk: {}", file.path);
    }

    // Update any episodes that reference this file to clear the reference
    let episodes = episode_repo
        .get_by_series_id(file.series_id)
        .await
        .unwrap_or_default();

    for mut ep in episodes {
        if ep.episode_file_id == Some(id) {
            ep.episode_file_id = None;
            ep.has_file = false;
            if let Err(e) = episode_repo.update(&ep).await {
                tracing::warn!(
                    "Failed to update episode {} after file deletion: {}",
                    ep.id,
                    e
                );
            }
        }
    }

    // Delete the database record
    file_repo.delete(id).await.map_err(|e| {
        EpisodeFileError::Internal(format!("Failed to delete episode file record: {}", e))
    })?;

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

    let mut file = repo
        .get_by_id(id)
        .await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode file: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    // Update fields that can be modified
    file.release_group = body.release_group.clone();
    file.scene_name = body.scene_name.clone();
    file.quality = serde_json::to_string(&body.quality).unwrap_or_default().into();
    file.languages = serde_json::to_string(&body.languages).unwrap_or_default().into();

    repo.update(&file)
        .await
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

    let mut disk_errors = 0;

    // Batch fetch all files upfront instead of N individual queries
    let files = file_repo
        .get_by_ids(&body.episode_file_ids)
        .await
        .unwrap_or_default();

    for file in &files {
        // Delete the actual file from disk
        let file_path = std::path::Path::new(&file.path);
        if file_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&file.path).await {
                tracing::warn!("Failed to delete file from disk: {} - {}", file.path, e);
                disk_errors += 1;
            }
        }
    }

    // Batch unlink episodes referencing these files
    let file_ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    if !file_ids.is_empty() {
        let pool = state.db.pool();
        let _ = sqlx::query(
            "UPDATE episodes SET episode_file_id = NULL, has_file = false WHERE episode_file_id = ANY($1)",
        )
        .bind(&file_ids)
        .execute(pool)
        .await;

        // Batch delete DB records
        let _ = sqlx::query("DELETE FROM episode_files WHERE id = ANY($1)")
            .bind(&file_ids)
            .execute(pool)
            .await;
    }

    let deleted_count = files.len();

    tracing::info!(
        "Bulk deleted {} episode files ({} disk errors)",
        deleted_count,
        disk_errors
    );

    if deleted_count > 0 {
        crate::core::logging::log_info(
            "EpisodeFilesDeleted",
            &format!("Bulk deleted {} episode files", deleted_count),
        )
        .await;
    }

    Ok(Json(serde_json::json!({})))
}

// ────────────────────────────────────────────────────────────
// Rename preview & execute
// ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewItem {
    pub episode_file_id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub existing_path: String,
    pub new_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameQuery {
    pub series_id: i64,
}

/// GET /api/v5/rename?seriesId=X — Preview which files would be renamed
pub async fn get_rename_preview(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RenameQuery>,
) -> Result<Json<Vec<RenamePreviewItem>>, EpisodeFileError> {
    let series_repo = SeriesRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());
    let file_repo = EpisodeFileRepository::new(state.db.clone());

    let series = series_repo
        .get_by_id(query.series_id)
        .await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    let episodes = episode_repo
        .get_by_series_id(query.series_id)
        .await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episodes: {}", e)))?;

    let files = file_repo
        .get_by_series_id(query.series_id)
        .await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch episode files: {}", e)))?;

    let config = state.config.read().media.clone();
    let mut previews = Vec::new();

    for file in &files {
        // Find episodes linked to this file
        let file_episodes: Vec<_> = episodes
            .iter()
            .filter(|e| e.episode_file_id == Some(file.id))
            .cloned()
            .collect();

        if file_episodes.is_empty() {
            continue;
        }

        // Parse quality from the stored JSON
        let quality: crate::core::profiles::qualities::QualityModel =
            serde_json::from_str(&file.quality).unwrap_or_default();

        let ctx = crate::core::naming::EpisodeNamingContext {
            series: &series,
            episodes: &file_episodes,
            quality: &quality,
            release_group: file.release_group.as_deref(),
        };

        // Build the expected filename
        let new_filename = crate::core::naming::build_episode_filename(&config, &ctx);
        let season_folder = crate::core::naming::build_season_folder(&config, file.season_number);

        // Get the file extension from the current path
        let ext = std::path::Path::new(&file.path)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        let new_relative = format!("{}/{}{}", season_folder, new_filename, ext);
        let new_absolute = format!("{}/{}", series.path, new_relative);

        // Only include if the path is actually different
        if new_absolute != file.path {
            let episode_numbers: Vec<i32> =
                file_episodes.iter().map(|e| e.episode_number).collect();

            previews.push(RenamePreviewItem {
                episode_file_id: file.id,
                series_id: file.series_id,
                season_number: file.season_number,
                episode_numbers,
                existing_path: file.path.clone(),
                new_path: new_absolute,
            });
        }
    }

    // Sort by season, then episode number
    previews.sort_by(|a, b| {
        a.season_number
            .cmp(&b.season_number)
            .then_with(|| a.episode_numbers.cmp(&b.episode_numbers))
    });

    Ok(Json(previews))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub series_id: i64,
    pub files: Vec<RenameFileEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFileEntry {
    pub episode_file_id: i64,
    pub new_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameResult {
    pub renamed: i32,
    pub failed: i32,
    pub errors: Vec<String>,
}

/// PUT /api/v5/rename — Execute file renames on disk and update DB
pub async fn execute_rename(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<RenameResult>, EpisodeFileError> {
    let file_repo = EpisodeFileRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());

    let series = series_repo
        .get_by_id(body.series_id)
        .await
        .map_err(|e| EpisodeFileError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(EpisodeFileError::NotFound)?;

    // Clean up orphan episode_file records (not referenced by any episode)
    let orphan_cleanup = sqlx::query(
        "DELETE FROM episode_files WHERE series_id = $1
         AND id NOT IN (SELECT episode_file_id FROM episodes WHERE episode_file_id IS NOT NULL AND series_id = $1)"
    )
    .bind(body.series_id)
    .execute(state.db.pool())
    .await;
    if let Ok(result) = orphan_cleanup {
        let count = result.rows_affected();
        if count > 0 {
            tracing::info!(
                "Cleaned up {} orphan episode_file records for series {}",
                count,
                body.series_id
            );
        }
    }

    // Dispatch to worker when Redis is available — worker has local disk access
    if let Some(ref hybrid_bus) = state.hybrid_event_bus {
        if hybrid_bus.is_redis_enabled() {
            if let Some(consumer) = state.scan_result_consumer.get() {
                let mut file_specs = Vec::new();
                let mut episode_file_ids = Vec::new();

                for entry in &body.files {
                    if let Ok(Some(file)) = file_repo.get_by_id(entry.episode_file_id).await {
                        if file.path != entry.new_path {
                            file_specs.push(crate::core::messaging::ImportFileSpec {
                                source_path: std::path::PathBuf::from(&file.path),
                                dest_path: std::path::PathBuf::from(&entry.new_path),
                            });
                            episode_file_ids.push(entry.episode_file_id);
                        }
                    }
                }

                if file_specs.is_empty() {
                    return Ok(Json(RenameResult {
                        renamed: 0,
                        failed: 0,
                        errors: vec![],
                    }));
                }

                let job_id = uuid::Uuid::new_v4().to_string();
                let count = file_specs.len() as i32;

                consumer
                    .register_rename_job(&job_id, episode_file_ids.clone())
                    .await;

                let message = crate::core::messaging::Message::RenameFilesRequest {
                    job_id: job_id.clone(),
                    files: file_specs,
                    episode_file_ids,
                };
                hybrid_bus.enqueue_job(message).await;

                tracing::info!(
                    "Rename dispatched to worker: {} file(s) for series '{}' (job_id={})",
                    count,
                    series.title,
                    job_id,
                );

                return Ok(Json(RenameResult {
                    renamed: count,
                    failed: 0,
                    errors: vec![],
                }));
            }
        }
    }

    // Fallback: rename locally (server has NFS access to files)
    let mut renamed = 0i32;
    let mut failed = 0i32;
    let mut errors = Vec::new();

    for entry in &body.files {
        let file = match file_repo.get_by_id(entry.episode_file_id).await {
            Ok(Some(f)) => f,
            Ok(None) => {
                errors.push(format!("File {} not found", entry.episode_file_id));
                failed += 1;
                continue;
            }
            Err(e) => {
                errors.push(format!(
                    "Failed to fetch file {}: {}",
                    entry.episode_file_id, e
                ));
                failed += 1;
                continue;
            }
        };

        let old_path = std::path::Path::new(&file.path);
        let new_path = std::path::Path::new(&entry.new_path);

        // Ensure the target directory exists
        if let Some(parent) = new_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                errors.push(format!(
                    "Failed to create directory for {}: {}",
                    entry.new_path, e
                ));
                failed += 1;
                continue;
            }
        }

        // Move the file on disk
        if old_path.exists() {
            if new_path.exists() {
                // Destination already exists — check if it's the same file via hash
                match (
                    crate::core::mediafiles::compute_file_hash(old_path).await,
                    crate::core::mediafiles::compute_file_hash(new_path).await,
                ) {
                    (Ok(src_hash), Ok(dst_hash)) if src_hash == dst_hash => {
                        // Same content — remove the misnamed source file
                        if let Err(e) = tokio::fs::remove_file(old_path).await {
                            errors.push(format!(
                                "Destination exists with matching hash but failed to remove source {}: {}",
                                file.path, e
                            ));
                            failed += 1;
                            continue;
                        }
                        tracing::info!(
                            "Removed duplicate source file {} (hash matches destination {})",
                            file.path,
                            entry.new_path
                        );
                    }
                    (Ok(_), Ok(_)) => {
                        // Different content — conflict, don't overwrite
                        errors.push(format!(
                            "Destination already exists with different content: {}",
                            entry.new_path
                        ));
                        failed += 1;
                        continue;
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        errors.push(format!(
                            "Failed to hash files for dedup check {} -> {}: {}",
                            file.path, entry.new_path, e
                        ));
                        failed += 1;
                        continue;
                    }
                }
            } else if let Err(e) = tokio::fs::rename(old_path, new_path).await {
                errors.push(format!(
                    "Failed to rename {} -> {}: {}",
                    file.path, entry.new_path, e
                ));
                failed += 1;
                continue;
            }
        } else if new_path.exists() {
            // Source is gone but destination exists — file was likely already moved
            tracing::info!(
                "Source file missing but destination exists, updating DB path: {} -> {}",
                file.path,
                entry.new_path
            );
        } else {
            errors.push(format!("Source file does not exist: {}", file.path));
            failed += 1;
            continue;
        }

        // Compute new relative path (relative to series root)
        let new_relative = new_path
            .strip_prefix(&series.path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.new_path.clone());

        // Update DB record
        let mut updated_file = file.clone();
        updated_file.path = entry.new_path.clone();
        updated_file.relative_path = new_relative;

        if let Err(e) = file_repo.update(&updated_file).await {
            errors.push(format!(
                "File moved but DB update failed for {}: {}",
                entry.episode_file_id, e
            ));
            failed += 1;
            continue;
        }

        tracing::info!(
            "Renamed episode file {}: {} -> {}",
            entry.episode_file_id,
            file.path,
            entry.new_path
        );
        renamed += 1;
    }

    // --- Orphan file cleanup ---
    // After renames, scan the series folder for video files not tracked in the DB.
    // If an orphan file hash-matches a tracked file, it's a leftover duplicate — delete it.
    let mut orphans_removed = 0i32;
    let tracked_files = file_repo
        .get_by_series_id(body.series_id)
        .await
        .unwrap_or_default();
    let tracked_paths: std::collections::HashSet<String> =
        tracked_files.iter().map(|f| f.path.clone()).collect();

    if let Ok(mut walker) = tokio::fs::read_dir(&series.path).await {
        let mut orphan_paths = Vec::new();
        while let Ok(Some(entry)) = walker.next_entry().await {
            let path = entry.path();
            if path.is_file() && crate::core::scanner::is_video_file(&path) {
                let path_str = path.to_string_lossy().to_string();
                if !tracked_paths.contains(&path_str) {
                    orphan_paths.push(path);
                }
            }
        }

        if !orphan_paths.is_empty() {
            // Pre-compute hashes of tracked files for comparison (only those on disk)
            let mut tracked_hashes: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for tf in &tracked_files {
                let tf_path = std::path::Path::new(&tf.path);
                if tf_path.exists() {
                    if let Ok(h) = crate::core::mediafiles::compute_file_hash(tf_path).await {
                        tracked_hashes.insert(h);
                    }
                }
            }

            for orphan in &orphan_paths {
                match crate::core::mediafiles::compute_file_hash(orphan).await {
                    Ok(hash) if tracked_hashes.contains(&hash) => {
                        if let Err(e) = tokio::fs::remove_file(orphan).await {
                            errors.push(format!(
                                "Failed to remove orphan duplicate {}: {}",
                                orphan.display(),
                                e
                            ));
                        } else {
                            tracing::info!(
                                "Removed orphan duplicate: {} (hash matches tracked file)",
                                orphan.display()
                            );
                            orphans_removed += 1;
                        }
                    }
                    _ => {
                        // Not a duplicate or hash failed — leave it alone
                    }
                }
            }
        }
    }

    if renamed > 0 || orphans_removed > 0 {
        let msg = if orphans_removed > 0 {
            format!(
                "Renamed {} episode files, removed {} orphan duplicates for series '{}'",
                renamed, orphans_removed, series.title
            )
        } else {
            format!(
                "Renamed {} episode files for series '{}'",
                renamed, series.title
            )
        };
        crate::core::logging::log_info("EpisodeFilesRenamed", &msg).await;
    }

    Ok(Json(RenameResult {
        renamed,
        failed,
        errors,
    }))
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
            EpisodeFileError::NotFound => {
                (StatusCode::NOT_FOUND, "Episode file not found".to_string())
            }
            EpisodeFileError::Internal(msg) => {
                tracing::error!("Episode file error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
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
        .route("/rename", get(get_rename_preview).put(execute_rename))
}
