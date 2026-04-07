//! Per-content-type queue endpoints.
//!
//! These provide filtered views of the queue for each content type:
//! `/queue/series`, `/queue/movies`, `/queue/music`, `/queue/audiobooks`, `/queue/podcasts`
//!
//! Each endpoint returns the same `QueueResponse` shape as the unified `/queue`
//! endpoint, but filtered to only include items of the specified content type.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::Json,
};

use super::common::*;
use super::fetch::fetch_all_downloads;
use crate::core::datastore::repositories::{
    ArtistRepository, AudiobookRepository, EpisodeRepository, MovieRepository, SeriesRepository,
};
use crate::web::AppState;

/// Filter queue items by content type and build a `QueueResponse`.
async fn filtered_queue(
    state: &AppState,
    params: &QueueListQuery,
    content_types: &[&str],
) -> QueueResponse {
    let include_unknown = params.include_unknown_series_items.unwrap_or(true);
    let mut all_downloads = fetch_all_downloads(state, include_unknown).await;

    // Filter to requested content types
    all_downloads.retain(|dl| content_types.contains(&dl.content_type.as_str()));

    // Enrich with metadata
    let include_episode = params.include_episode.unwrap_or(true);
    let include_series = params.include_series.unwrap_or(true);

    if include_episode || include_series {
        let episode_repo = EpisodeRepository::new(state.db.clone());
        let series_repo = SeriesRepository::new(state.db.clone());
        let movie_repo = MovieRepository::new(state.db.clone());
        let artist_repo = ArtistRepository::new(state.db.clone());
        let audiobook_repo = AudiobookRepository::new(state.db.clone());

        for dl in &mut all_downloads {
            if include_episode {
                if let Some(ep_id) = dl.episode_id {
                    if let Ok(Some(ep)) = episode_repo.get_by_id(ep_id).await {
                        dl.episode = Some(QueueEpisodeResource {
                            id: ep.id,
                            season_number: ep.season_number,
                            episode_number: ep.episode_number,
                            title: ep.title,
                            air_date_utc: ep.air_date_utc.map(|d| d.to_rfc3339()),
                        });
                    }
                }
            }
            if include_series {
                if let Some(sid) = dl.series_id {
                    if let Ok(Some(s)) = series_repo.get_by_id(sid).await {
                        if s.series_type == 2 {
                            dl.content_type = "anime".to_string();
                        }
                        dl.series = Some(QueueSeriesResource {
                            id: s.id,
                            title_slug: s.title_slug.clone(),
                            title: s.title,
                        });
                    }
                }
            }
            if let Some(mid) = dl.movie_id {
                if let Ok(Some(m)) = movie_repo.get_by_id(mid).await {
                    dl.movie = Some(QueueMovieResource {
                        id: m.id,
                        title: m.title,
                        title_slug: m.title_slug,
                    });
                }
            }
            if let Some(aid) = dl.artist_id {
                if let Ok(Some(a)) = artist_repo.get_by_id(aid).await {
                    dl.artist = Some(QueueArtistResource {
                        id: a.id,
                        title: a.name,
                        title_slug: a.title_slug,
                    });
                }
            }
            if let Some(abid) = dl.audiobook_id {
                if let Ok(Some(ab)) = audiobook_repo.get_by_id(abid).await {
                    dl.audiobook = Some(QueueAudiobookResource {
                        id: ab.id,
                        title: ab.title,
                        title_slug: ab.title_slug,
                    });
                }
            }
        }
    }

    // Re-filter after enrichment (anime detection may have changed content_type)
    all_downloads.retain(|dl| content_types.contains(&dl.content_type.as_str()));

    // Enrich importing items with live progress
    if let Some(consumer) = state.scan_result_consumer.get() {
        let import_progress = consumer.get_import_progress_by_download_id().await;
        if !import_progress.is_empty() {
            for dl in &mut all_downloads {
                if dl.tracked_download_state.as_deref() == Some("importing") {
                    if let Some(ref download_id) = dl.download_id {
                        if let Some(progress) = import_progress.get(download_id) {
                            dl.import_progress = Some(ImportProgressResource {
                                stage: progress.stage.clone(),
                                current_file: progress.current_file.clone(),
                                files_total: progress.files_total,
                                files_processed: progress.files_processed,
                                percent: progress.percent,
                                detail: progress.detail.clone(),
                                bytes_copied: progress.bytes_copied,
                                bytes_total: progress.bytes_total,
                            });
                        }
                    }
                }
            }
        }
    }

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(10000).clamp(1, 10000);
    let total_records = all_downloads.len() as i64;

    let start = ((page - 1) * page_size) as usize;
    let end = (start + page_size as usize).min(all_downloads.len());

    let records = if start < all_downloads.len() {
        all_downloads[start..end].to_vec()
    } else {
        vec![]
    };

    QueueResponse {
        page,
        page_size,
        sort_key: params.sort_key.clone().unwrap_or_else(|| "timeleft".to_string()),
        sort_direction: params
            .sort_direction
            .clone()
            .unwrap_or_else(|| "ascending".to_string()),
        total_records,
        records,
        hidden_imported_count: 0,
        completed_records: vec![],
    }
}

/// GET /queue/series — series downloads only
pub(super) async fn list_series_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(filtered_queue(&state, &params, &["series"]).await)
}

/// GET /queue/movies — movie downloads only
pub(super) async fn list_movies_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(filtered_queue(&state, &params, &["movie"]).await)
}

/// GET /queue/anime — anime downloads only
pub(super) async fn list_anime_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(filtered_queue(&state, &params, &["anime"]).await)
}

/// GET /queue/music — music downloads only
pub(super) async fn list_music_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(filtered_queue(&state, &params, &["music"]).await)
}

/// GET /queue/audiobooks — audiobook downloads only
pub(super) async fn list_audiobooks_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(filtered_queue(&state, &params, &["audiobook"]).await)
}

/// GET /queue/podcasts — podcast downloads only
pub(super) async fn list_podcasts_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(filtered_queue(&state, &params, &["podcast"]).await)
}

/// GET /queue/completed — recently completed downloads derived from history table.
///
/// Returns Grabbed and Imported history events as QueueResource items for the
/// "Completed" tab.  This replaces the old approach of storing terminal-state
/// records (status=4 Imported, status=7 Ignored) in the tracked_downloads table.
pub(super) async fn list_completed_queue(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<QueueResource>> {
    use crate::core::datastore::repositories::HistoryRepository;

    let history_repo = HistoryRepository::new(state.db.clone());
    let completed = history_repo.get_recent_completed(100).await.unwrap_or_default();

    let records: Vec<QueueResource> = completed
        .iter()
        .map(|h| {
            let core_quality: crate::core::profiles::qualities::QualityModel =
                serde_json::from_str(&h.quality).unwrap_or_default();
            let quality = QualityModel {
                quality: QualityResource {
                    id: core_quality.quality.weight(),
                    name: format!("{:?}", core_quality.quality),
                    source: "unknown".to_string(),
                    resolution: core_quality.quality.resolution_width(),
                },
                revision: RevisionResource {
                    version: core_quality.revision.version,
                    real: core_quality.revision.real,
                    is_repack: core_quality.revision.is_repack,
                },
            };

            let event_state = if h.event_type == 3 {
                "imported"
            } else {
                "grabbed"
            };

            // Determine content type from which FK is set
            let content_type = if h.movie_id.is_some_and(|id| id > 0) {
                "movie"
            } else {
                "series"
            };

            QueueResource {
                id: h.id,
                series_id: h.series_id.filter(|&id| id > 0),
                episode_id: h.episode_id.filter(|&id| id > 0),
                languages: vec![LanguageResource {
                    id: 1,
                    name: "English".to_string(),
                }],
                quality,
                custom_formats: vec![],
                custom_format_score: 0,
                size: 0.0,
                title: h.source_title.clone(),
                sizeleft: 0.0,
                timeleft: None,
                estimated_completion_time: None,
                added: Some(h.date.to_rfc3339()),
                status: "completed".to_string(),
                tracked_download_status: Some("ok".to_string()),
                tracked_download_state: Some(event_state.to_string()),
                status_messages: vec![],
                error_message: None,
                download_id: h.download_id.clone(),
                protocol: "unknown".to_string(),
                download_client: None,
                download_client_has_post_import_category: false,
                indexer: None,
                output_path: None,
                episode_has_file: true,
                content_type: content_type.to_string(),
                movie_id: h.movie_id.filter(|&id| id > 0),
                artist_id: None,
                audiobook_id: None,
                album_id: None,
                seeds: None,
                leechers: None,
                seed_count: None,
                leech_count: None,
                episode: None,
                series: None,
                movie: None,
                artist: None,
                audiobook: None,
                import_progress: None,
            }
        })
        .collect();

    Json(records)
}
