//! Queue list, status, details, and item retrieval handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};

use super::common::*;
use super::fetch::fetch_all_downloads;
use crate::core::datastore::repositories::{
    ArtistRepository, AudiobookRepository, EpisodeRepository, MovieRepository, SeriesRepository,
    TrackedDownloadRepository,
};
use crate::web::AppState;

pub(super) async fn list_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    let include_unknown = params.include_unknown_series_items.unwrap_or(true);
    let include_episode = params.include_episode.unwrap_or(true);
    let include_series = params.include_series.unwrap_or(true);
    let mut all_downloads = fetch_all_downloads(&state, include_unknown).await;

    // Fetch completed tracked downloads (Imported=4 + Ignored=7)
    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let imported_records = td_repo.get_completed().await.unwrap_or_default();
    let hidden_imported_count = imported_records.len() as i64;

    // Build completed records from imported tracked downloads
    let mut completed_records: Vec<QueueResource> = imported_records
        .iter()
        .map(|td| {
            let protocol_str = match td.protocol {
                1 => "usenet",
                2 => "torrent",
                _ => "unknown",
            };
            let core_quality: crate::core::profiles::qualities::QualityModel =
                serde_json::from_str(&td.quality).unwrap_or_default();
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
            let languages = vec![LanguageResource {
                id: 1,
                name: "English".to_string(),
            }];
            let episode_ids: Vec<i64> = serde_json::from_str(&td.episode_ids).unwrap_or_default();
            let content_type = td.content_type.clone();

            QueueResource {
                id: td.id,
                series_id: if td.series_id > 0 {
                    Some(td.series_id)
                } else {
                    None
                },
                episode_id: episode_ids.first().copied(),
                languages,
                quality,
                custom_formats: vec![],
                custom_format_score: 0,
                size: td.size as f64,
                title: td.title.clone(),
                sizeleft: 0.0,
                timeleft: None,
                estimated_completion_time: None,
                added: Some(td.added.to_rfc3339()),
                status: "completed".to_string(),
                tracked_download_status: Some("ok".to_string()),
                tracked_download_state: Some(
                    if td.status == 7 {
                        "ignored"
                    } else {
                        "imported"
                    }
                    .to_string(),
                ),
                status_messages: vec![],
                error_message: None,
                download_id: Some(td.download_id.clone()),
                protocol: protocol_str.to_string(),
                download_client: None,
                download_client_has_post_import_category: false,
                indexer: td.indexer.clone(),
                output_path: td.output_path.clone(),
                episode_has_file: true,
                content_type,
                movie_id: td.movie_id,
                artist_id: td.artist_id,
                audiobook_id: td.audiobook_id,
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

    // Enrich with episode/series/movie metadata
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
                        // Detect anime from series_type (2 = anime)
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
            // Enrich movie data
            if let Some(mid) = dl.movie_id {
                if let Ok(Some(m)) = movie_repo.get_by_id(mid).await {
                    dl.movie = Some(QueueMovieResource {
                        id: m.id,
                        title: m.title,
                        title_slug: m.title_slug,
                    });
                }
            }
            // Enrich artist data (music)
            if let Some(aid) = dl.artist_id {
                if let Ok(Some(a)) = artist_repo.get_by_id(aid).await {
                    dl.artist = Some(QueueArtistResource {
                        id: a.id,
                        title: a.name,
                        title_slug: a.title_slug,
                    });
                }
            }
            // Enrich audiobook data
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

        // Enrich completed records with the same metadata
        for dl in &mut completed_records {
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

    // Enrich importing items with live import progress from the scan result consumer
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

    Json(QueueResponse {
        page,
        page_size,
        sort_key: params.sort_key.unwrap_or_else(|| "timeleft".to_string()),
        sort_direction: params
            .sort_direction
            .unwrap_or_else(|| "ascending".to_string()),
        total_records,
        records,
        hidden_imported_count,
        completed_records,
    })
}

pub(super) async fn get_queue_status(
    State(state): State<Arc<AppState>>,
) -> Json<QueueStatusResource> {
    let downloads = fetch_all_downloads(&state, true).await;

    let total_count = downloads.len() as i32;
    let errors = downloads.iter().any(|d| d.status == "failed");
    let warnings = downloads.iter().any(|d| d.status == "warning");

    Json(QueueStatusResource {
        total_count,
        count: total_count,
        unknown_count: total_count,
        errors,
        warnings,
        unknown_errors: errors,
        unknown_warnings: warnings,
    })
}

pub(super) async fn get_queue_details(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<QueueResource>> {
    let downloads = fetch_all_downloads(&state, true).await;
    Json(downloads)
}

pub(super) async fn get_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<QueueResource>, StatusCode> {
    let downloads = fetch_all_downloads(&state, true).await;
    downloads
        .into_iter()
        .find(|d| d.id == id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
