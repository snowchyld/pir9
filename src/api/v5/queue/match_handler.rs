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
    SeriesRepository,
};
use crate::core::queue::tracked::{
    AudiobookRef, MovieRef, MusicRef, SeriesRef, TrackedDownload,
};
use crate::core::queue::UNTRACKED_ID_BASE;
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
    /// Required for untracked downloads (id >= UNTRACKED_ID_BASE) — the download client's ID for this item
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
/// For tracked downloads (id < UNTRACKED_ID_BASE): updates the existing in-memory record.
/// For untracked downloads (id >= UNTRACKED_ID_BASE): promotes to a tracked download by
/// creating a new record in the appropriate store.
pub(super) async fn update_match(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateMatchRequest>,
) -> Result<Json<QueueActionResponse>, StatusCode> {
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

        if id < UNTRACKED_ID_BASE {
            // Tracked download — update in audiobooks store (or migrate if in different store)
            let updated = state
                .tracked
                .audiobooks
                .update(id, |td| {
                    td.content.audiobook_id = audiobook_id;
                })
                .await
                .unwrap_or(false);
            if !updated {
                // Download may be in a different store — remove and re-insert as audiobook
                if let Some(existing) = state.tracked.find_by_id(id).await {
                    state.tracked.remove_by_id(id).await;
                    let td = TrackedDownload {
                        id: 0,
                        download_id: existing.download_id,
                        client_id: existing.client_id,
                        content: AudiobookRef { audiobook_id },
                        title: existing.title,
                        quality: existing.quality,
                        indexer: existing.indexer,
                        added: existing.added,
                        is_upgrade: existing.is_upgrade,
                    };
                    if let Err(e) = state.tracked.audiobooks.insert(td).await {
                        tracing::warn!("Failed to update audiobook match: {}", e);
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                } else {
                    tracing::warn!("Tracked download {} not found", id);
                    return Ok(Json(QueueActionResponse { success: false }));
                }
            }
            tracing::info!(
                "Queue match updated: download {} → audiobook {}",
                id,
                audiobook_id
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

            // Check if already tracked by download_id
            if let Some(existing) = state
                .tracked
                .audiobooks
                .get_by_download_id(client_id, &download_id)
                .await
            {
                let _ = state
                    .tracked
                    .audiobooks
                    .update(existing.id, |td| {
                        td.content.audiobook_id = audiobook_id;
                    })
                    .await;
            } else {
                let td = TrackedDownload {
                    id: 0,
                    download_id: download_id.clone(),
                    client_id,
                    content: AudiobookRef { audiobook_id },
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    quality: "{}".to_string(),
                    added: chrono::Utc::now(),
                    is_upgrade: false,
                };
                if let Err(e) = state.tracked.audiobooks.insert(td).await {
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

        if id < UNTRACKED_ID_BASE {
            // Tracked download — update in music store (or migrate if in different store)
            let updated = state
                .tracked
                .music
                .update(id, |td| {
                    td.content.artist_id = artist_id;
                })
                .await
                .unwrap_or(false);
            if !updated {
                if let Some(existing) = state.tracked.find_by_id(id).await {
                    state.tracked.remove_by_id(id).await;
                    let td = TrackedDownload {
                        id: 0,
                        download_id: existing.download_id,
                        client_id: existing.client_id,
                        content: MusicRef { artist_id },
                        title: existing.title,
                        quality: existing.quality,
                        indexer: existing.indexer,
                        added: existing.added,
                        is_upgrade: existing.is_upgrade,
                    };
                    if let Err(e) = state.tracked.music.insert(td).await {
                        tracing::warn!("Failed to update artist match: {}", e);
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                } else {
                    tracing::warn!("Tracked download {} not found", id);
                    return Ok(Json(QueueActionResponse { success: false }));
                }
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

            if let Some(existing) = state
                .tracked
                .music
                .get_by_download_id(client_id, &download_id)
                .await
            {
                let _ = state
                    .tracked
                    .music
                    .update(existing.id, |td| {
                        td.content.artist_id = artist_id;
                    })
                    .await;
                tracing::info!(
                    "Queue match updated (existing): download {} → artist {}",
                    existing.id,
                    artist_id
                );
            } else {
                let td = TrackedDownload {
                    id: 0,
                    download_id: download_id.clone(),
                    client_id,
                    content: MusicRef { artist_id },
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    quality: "{}".to_string(),
                    added: chrono::Utc::now(),
                    is_upgrade: false,
                };
                match state.tracked.music.insert(td).await {
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

        if id < UNTRACKED_ID_BASE {
            // Tracked download — update in movies store (or migrate if in different store)
            let updated = state
                .tracked
                .movies
                .update(id, |td| {
                    td.content.movie_id = movie_id;
                })
                .await
                .unwrap_or(false);
            if !updated {
                if let Some(existing) = state.tracked.find_by_id(id).await {
                    state.tracked.remove_by_id(id).await;
                    let td = TrackedDownload {
                        id: 0,
                        download_id: existing.download_id,
                        client_id: existing.client_id,
                        content: MovieRef { movie_id },
                        title: existing.title,
                        quality: existing.quality,
                        indexer: existing.indexer,
                        added: existing.added,
                        is_upgrade: existing.is_upgrade,
                    };
                    if let Err(e) = state.tracked.movies.insert(td).await {
                        tracing::warn!("Failed to update movie match: {}", e);
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                } else {
                    tracing::warn!("Tracked download {} not found", id);
                    return Ok(Json(QueueActionResponse { success: false }));
                }
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

            if let Some(existing) = state
                .tracked
                .movies
                .get_by_download_id(client_id, &download_id)
                .await
            {
                let _ = state
                    .tracked
                    .movies
                    .update(existing.id, |td| {
                        td.content.movie_id = movie_id;
                    })
                    .await;
                tracing::info!(
                    "Queue match updated (existing): download {} → movie {}",
                    existing.id,
                    movie_id
                );
            } else {
                let td = TrackedDownload {
                    id: 0,
                    download_id: download_id.clone(),
                    client_id,
                    content: MovieRef { movie_id },
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    quality: "{}".to_string(),
                    added: chrono::Utc::now(),
                    is_upgrade: false,
                };
                match state.tracked.movies.insert(td).await {
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

        if id < UNTRACKED_ID_BASE {
            // Tracked download — update in series store (or migrate if in different store)
            let updated = state
                .tracked
                .series
                .update(id, |td| {
                    td.content.series_id = series_id;
                    td.content.episode_ids = episode_ids.clone();
                })
                .await
                .unwrap_or(false);
            if !updated {
                if let Some(existing) = state.tracked.find_by_id(id).await {
                    state.tracked.remove_by_id(id).await;
                    let td = TrackedDownload {
                        id: 0,
                        download_id: existing.download_id,
                        client_id: existing.client_id,
                        content: SeriesRef {
                            series_id,
                            episode_ids: episode_ids.clone(),
                        },
                        title: existing.title,
                        quality: existing.quality,
                        indexer: existing.indexer,
                        added: existing.added,
                        is_upgrade: existing.is_upgrade,
                    };
                    if let Err(e) = state.tracked.series.insert(td).await {
                        tracing::warn!("Failed to update series match: {}", e);
                        return Ok(Json(QueueActionResponse { success: false }));
                    }
                } else {
                    tracing::warn!("Tracked download {} not found", id);
                    return Ok(Json(QueueActionResponse { success: false }));
                }
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

            // Check if a tracked download already exists for this download
            if let Some(existing) = state
                .tracked
                .series
                .get_by_download_id(client_id, &download_id)
                .await
            {
                let _ = state
                    .tracked
                    .series
                    .update(existing.id, |td| {
                        td.content.series_id = series_id;
                        td.content.episode_ids = episode_ids.clone();
                    })
                    .await;

                tracing::info!(
                    "Queue match updated (existing): download {} → series {}, episodes {:?}",
                    existing.id,
                    series_id,
                    episode_ids,
                );
            } else {
                // Truly untracked — promote to tracked by creating a new record
                let td = TrackedDownload {
                    id: 0,
                    download_id: download_id.clone(),
                    client_id,
                    content: SeriesRef {
                        series_id,
                        episode_ids: episode_ids.clone(),
                    },
                    title: body.title.unwrap_or_default(),
                    indexer: None,
                    quality: "{}".to_string(),
                    added: chrono::Utc::now(),
                    is_upgrade: false,
                };

                match state.tracked.series.insert(td).await {
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
