//! Match update handler — manually fix series/episode or movie match for a queue item.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;

use super::common::QueueActionResponse;
use crate::core::datastore::repositories::{
    ArtistRepository, AudiobookRepository, DownloadClientRepository, MovieRepository,
    SeriesRepository, TrackedDownloadRepository,
};
use crate::core::queue::TrackedDownloadState;
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMatchRequest {
    /// Series ID (for series/anime match)
    pub series_id: Option<i64>,
    /// Episode IDs (for series/anime match)
    pub episode_ids: Option<Vec<i64>>,
    /// Movie ID (for movie match — mutually exclusive with series_id)
    pub movie_id: Option<i64>,
    /// Artist ID (for music match — mutually exclusive with series_id and movie_id)
    pub artist_id: Option<i64>,
    /// Audiobook ID (for audiobook match)
    pub audiobook_id: Option<i64>,
    /// Required for untracked downloads (id >= 10000) — the download client's ID for this item
    pub download_id: Option<String>,
    /// Required for untracked downloads — the download client name
    pub download_client: Option<String>,
    /// Protocol: "usenet" or "torrent"
    pub protocol: Option<String>,
    /// Download size in bytes
    pub size: Option<f64>,
    /// Release title
    pub title: Option<String>,
}

/// PUT /api/v5/queue/{id}/match
/// Manually fix the series/episode or movie match for a queue item.
/// For tracked downloads (id < 10000): updates the existing DB record.
/// For untracked downloads (id >= 10000): promotes to a tracked download by
/// creating a new DB record with the corrected match.
pub(super) async fn update_match(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateMatchRequest>,
) -> Result<Json<QueueActionResponse>, StatusCode> {
    let td_repo = TrackedDownloadRepository::new(state.db.clone());

    // Determine match type: audiobook > artist > movie > series
    let is_audiobook_match = body.audiobook_id.is_some();
    let is_artist_match = body.artist_id.is_some();
    let is_movie_match = body.movie_id.is_some();

    if is_audiobook_match {
        // --- Audiobook match ---
        let audiobook_id = body.audiobook_id.unwrap();
        let audiobook_repo = AudiobookRepository::new(state.db.clone());

        match audiobook_repo.get_by_id(audiobook_id).await {
            Ok(Some(_)) => {}
            _ => return Err(StatusCode::NOT_FOUND),
        }

        if id < 10000 {
            if let Err(e) = td_repo.update_audiobook_match(id, audiobook_id).await {
                tracing::warn!(
                    "Failed to update audiobook match for tracked download {}: {}",
                    id,
                    e
                );
                return Ok(Json(QueueActionResponse { success: false }));
            }
            tracing::info!(
                "Queue match updated: download {} → audiobook {}",
                id,
                audiobook_id
            );
        } else {
            let download_id = match body.download_id {
                Some(ref id) if !id.is_empty() => id.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            let client_name = match body.download_client {
                Some(ref name) if !name.is_empty() => name.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };

            let client_repo = DownloadClientRepository::new(state.db.clone());
            let clients = client_repo.get_all().await.unwrap_or_default();
            let client_id = match clients.iter().find(|c| c.name == client_name) {
                Some(c) => c.id,
                None => return Err(StatusCode::NOT_FOUND),
            };

            if let Ok(Some(existing)) = td_repo.get_by_download_id(client_id, &download_id).await {
                if let Err(e) = td_repo
                    .update_audiobook_match(existing.id, audiobook_id)
                    .await
                {
                    tracing::warn!("Failed to update audiobook match: {}", e);
                    return Ok(Json(QueueActionResponse { success: false }));
                }
                let _ = td_repo
                    .update_status(
                        existing.id,
                        TrackedDownloadState::ImportPending as i32,
                        "[]",
                        None,
                    )
                    .await;
            } else {
                let protocol = match body.protocol.as_deref() {
                    Some("usenet") => 1,
                    _ => 2,
                };
                use crate::core::datastore::models::TrackedDownloadDbModel;
                let model = TrackedDownloadDbModel {
                    id: 0,
                    download_id: download_id.clone(),
                    download_client_id: client_id,
                    series_id: 0,
                    episode_ids: "[]".to_string(),
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    size: body.size.unwrap_or(0.0) as i64,
                    protocol,
                    quality: "{}".to_string(),
                    languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
                    status: TrackedDownloadState::Downloading as i32,
                    status_messages: "[]".to_string(),
                    error_message: None,
                    output_path: None,
                    is_upgrade: false,
                    added: chrono::Utc::now(),
                    movie_id: None,
                    artist_id: None,
                    audiobook_id: Some(audiobook_id),
                    content_type: "audiobook".to_string(),
                };
                if let Err(e) = td_repo.insert(&model).await {
                    tracing::warn!(
                        "Failed to promote untracked download '{}': {}",
                        download_id,
                        e
                    );
                    return Ok(Json(QueueActionResponse { success: false }));
                }
            }
        }
    } else if is_artist_match {
        // --- Artist match (music downloads) ---
        let artist_id = body.artist_id.unwrap();
        let artist_repo = ArtistRepository::new(state.db.clone());

        // Validate artist exists
        match artist_repo.get_by_id(artist_id).await {
            Ok(Some(_)) => {}
            _ => return Err(StatusCode::NOT_FOUND),
        }

        if id < 10000 {
            // Tracked download — update existing record
            if let Err(e) = td_repo.update_artist_match(id, artist_id).await {
                tracing::warn!(
                    "Failed to update artist match for tracked download {}: {}",
                    id,
                    e
                );
                return Ok(Json(QueueActionResponse { success: false }));
            }
            tracing::info!(
                "Queue match updated: download {} → artist {}",
                id,
                artist_id
            );
        } else {
            // Untracked download — promote to tracked
            let download_id = match body.download_id {
                Some(ref id) if !id.is_empty() => id.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            let client_name = match body.download_client {
                Some(ref name) if !name.is_empty() => name.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };

            let client_repo = DownloadClientRepository::new(state.db.clone());
            let clients = client_repo.get_all().await.unwrap_or_default();
            let client_id = match clients.iter().find(|c| c.name == client_name) {
                Some(c) => c.id,
                None => return Err(StatusCode::NOT_FOUND),
            };

            if let Ok(Some(existing)) = td_repo.get_by_download_id(client_id, &download_id).await {
                if let Err(e) = td_repo.update_artist_match(existing.id, artist_id).await {
                    tracing::warn!(
                        "Failed to update artist match for existing tracked download {}: {}",
                        existing.id,
                        e
                    );
                    return Ok(Json(QueueActionResponse { success: false }));
                }
                let _ = td_repo
                    .update_status(
                        existing.id,
                        TrackedDownloadState::ImportPending as i32,
                        "[]",
                        None,
                    )
                    .await;
                tracing::info!(
                    "Queue match updated (existing): download {} → artist {}",
                    existing.id,
                    artist_id
                );
            } else {
                let protocol = match body.protocol.as_deref() {
                    Some("usenet") => 1,
                    _ => 2,
                };
                use crate::core::datastore::models::TrackedDownloadDbModel;
                let model = TrackedDownloadDbModel {
                    id: 0,
                    download_id: download_id.clone(),
                    download_client_id: client_id,
                    series_id: 0,
                    episode_ids: "[]".to_string(),
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    size: body.size.unwrap_or(0.0) as i64,
                    protocol,
                    quality: "{}".to_string(),
                    languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
                    status: TrackedDownloadState::Downloading as i32,
                    status_messages: "[]".to_string(),
                    error_message: None,
                    output_path: None,
                    is_upgrade: false,
                    added: chrono::Utc::now(),
                    movie_id: None,
                    artist_id: Some(artist_id),
                    audiobook_id: None,
                    content_type: "music".to_string(),
                };
                match td_repo.insert(&model).await {
                    Ok(new_id) => {
                        tracing::info!(
                            "Untracked download promoted: '{}' → tracked {} (artist {})",
                            download_id,
                            new_id,
                            artist_id,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to promote untracked download '{}': {}",
                            download_id,
                            e
                        );
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                }
            }
        }
    } else if is_movie_match {
        // --- Movie match ---
        let movie_id = body.movie_id.unwrap();
        let movie_repo = MovieRepository::new(state.db.clone());

        // Validate movie exists
        match movie_repo.get_by_id(movie_id).await {
            Ok(Some(_)) => {}
            _ => return Err(StatusCode::NOT_FOUND),
        }

        if id < 10000 {
            // Tracked download — update existing record
            if let Err(e) = td_repo.update_movie_match(id, movie_id).await {
                tracing::warn!(
                    "Failed to update movie match for tracked download {}: {}",
                    id,
                    e
                );
                return Ok(Json(QueueActionResponse { success: false }));
            }
            tracing::info!("Queue match updated: download {} → movie {}", id, movie_id);
        } else {
            // Untracked download — promote to tracked
            let download_id = match body.download_id {
                Some(ref id) if !id.is_empty() => id.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            let client_name = match body.download_client {
                Some(ref name) if !name.is_empty() => name.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };

            let client_repo = DownloadClientRepository::new(state.db.clone());
            let clients = client_repo.get_all().await.unwrap_or_default();
            let client_id = match clients.iter().find(|c| c.name == client_name) {
                Some(c) => c.id,
                None => return Err(StatusCode::NOT_FOUND),
            };

            if let Ok(Some(existing)) = td_repo.get_by_download_id(client_id, &download_id).await {
                if let Err(e) = td_repo.update_movie_match(existing.id, movie_id).await {
                    tracing::warn!(
                        "Failed to update movie match for existing tracked download {}: {}",
                        existing.id,
                        e
                    );
                    return Ok(Json(QueueActionResponse { success: false }));
                }
                let _ = td_repo
                    .update_status(
                        existing.id,
                        TrackedDownloadState::ImportPending as i32,
                        "[]",
                        None,
                    )
                    .await;
                tracing::info!(
                    "Queue match updated (existing): download {} → movie {}",
                    existing.id,
                    movie_id
                );
            } else {
                let protocol = match body.protocol.as_deref() {
                    Some("usenet") => 1,
                    _ => 2,
                };
                use crate::core::datastore::models::TrackedDownloadDbModel;
                let model = TrackedDownloadDbModel {
                    id: 0,
                    download_id: download_id.clone(),
                    download_client_id: client_id,
                    series_id: 0,
                    episode_ids: "[]".to_string(),
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    size: body.size.unwrap_or(0.0) as i64,
                    protocol,
                    quality: "{}".to_string(),
                    languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
                    status: TrackedDownloadState::Downloading as i32,
                    status_messages: "[]".to_string(),
                    error_message: None,
                    output_path: None,
                    is_upgrade: false,
                    added: chrono::Utc::now(),
                    movie_id: Some(movie_id),
                    artist_id: None,
                    audiobook_id: None,
                    content_type: "movie".to_string(),
                };
                match td_repo.insert(&model).await {
                    Ok(new_id) => {
                        tracing::info!(
                            "Untracked download promoted: '{}' → tracked {} (movie {})",
                            download_id,
                            new_id,
                            movie_id,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to promote untracked download '{}': {}",
                            download_id,
                            e
                        );
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                }
            }
        }
    } else {
        // --- Series match ---
        // Only series is required; episode_ids are optional and resolved during import
        let series_id = match body.series_id {
            Some(sid) => sid,
            None => return Err(StatusCode::BAD_REQUEST),
        };
        let episode_ids = body.episode_ids.unwrap_or_default();

        let series_repo = SeriesRepository::new(state.db.clone());

        // Validate series exists
        match series_repo.get_by_id(series_id).await {
            Ok(Some(_)) => {}
            _ => return Err(StatusCode::NOT_FOUND),
        }

        let episode_ids_json =
            serde_json::to_string(&episode_ids).unwrap_or_else(|_| "[]".to_string());

        if id < 10000 {
            // Tracked download — update existing record
            if let Err(e) = td_repo
                .update_series_match(id, series_id, &episode_ids_json)
                .await
            {
                tracing::warn!("Failed to update match for tracked download {}: {}", id, e);
                return Ok(Json(QueueActionResponse { success: false }));
            }

            tracing::info!(
                "Queue match updated: download {} → series {}, episodes {:?}",
                id,
                series_id,
                episode_ids
            );
        } else {
            // Untracked download — either update existing record or create new one
            let download_id = match body.download_id {
                Some(ref id) if !id.is_empty() => id.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            let client_name = match body.download_client {
                Some(ref name) if !name.is_empty() => name.clone(),
                _ => return Err(StatusCode::BAD_REQUEST),
            };

            // Resolve download client ID from name
            let client_repo = DownloadClientRepository::new(state.db.clone());
            let clients = client_repo.get_all().await.unwrap_or_default();
            let client_id = match clients.iter().find(|c| c.name == client_name) {
                Some(c) => c.id,
                None => {
                    tracing::warn!("Match: download client '{}' not found", client_name);
                    return Err(StatusCode::NOT_FOUND);
                }
            };

            // Check if a tracked_downloads record already exists for this download.
            if let Ok(Some(existing)) = td_repo.get_by_download_id(client_id, &download_id).await {
                if let Err(e) = td_repo
                    .update_series_match(existing.id, series_id, &episode_ids_json)
                    .await
                {
                    tracing::warn!(
                        "Failed to update match for existing tracked download {}: {}",
                        existing.id,
                        e
                    );
                    return Ok(Json(QueueActionResponse { success: false }));
                }

                // Reset status to ImportPending
                let _ = td_repo
                    .update_status(
                        existing.id,
                        TrackedDownloadState::ImportPending as i32,
                        "[]",
                        None,
                    )
                    .await;

                tracing::info!(
                    "Queue match updated (existing): download {} → series {}, episodes {:?}",
                    existing.id,
                    series_id,
                    episode_ids,
                );
            } else {
                // Truly untracked — promote to tracked by creating a new record
                let protocol = match body.protocol.as_deref() {
                    Some("usenet") => 1,
                    _ => 2,
                };

                use crate::core::datastore::models::TrackedDownloadDbModel;

                let model = TrackedDownloadDbModel {
                    id: 0,
                    download_id: download_id.clone(),
                    download_client_id: client_id,
                    series_id,
                    episode_ids: episode_ids_json,
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    size: body.size.unwrap_or(0.0) as i64,
                    protocol,
                    quality: "{}".to_string(),
                    languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
                    status: TrackedDownloadState::Downloading as i32,
                    status_messages: "[]".to_string(),
                    error_message: None,
                    output_path: None,
                    is_upgrade: false,
                    added: chrono::Utc::now(),
                    movie_id: None,
                    artist_id: None,
                    audiobook_id: None,
                    content_type: "series".to_string(),
                };

                match td_repo.insert(&model).await {
                    Ok(new_id) => {
                        tracing::info!(
                            "Untracked download promoted: '{}' → tracked {} (series {}, episodes {:?})",
                            download_id,
                            new_id,
                            series_id,
                            episode_ids,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to promote untracked download '{}': {}",
                            download_id,
                            e
                        );
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                }
            }
        }
    }

    Ok(Json(QueueActionResponse { success: true }))
}
