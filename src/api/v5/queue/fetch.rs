//! Core download fetching — combines tracked + untracked downloads.

use std::collections::HashSet;

use super::common::*;
use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeRepository, MovieRepository, SeriesRepository,
    TrackedDownloadRepository,
};
use crate::core::download::clients::{create_client_from_model, DownloadState};
use crate::core::parser::{best_series_match, normalize_title, parse_title};
use crate::core::queue::{QueueResult, TrackedDownloadService, TrackedDownloadState};
use crate::web::AppState;

/// Fetch downloads from all enabled download clients.
/// Combines tracked downloads (from database) with untracked downloads (direct from clients).
pub(super) async fn fetch_all_downloads(
    state: &AppState,
    include_unknown: bool,
) -> Vec<QueueResource> {
    let service = TrackedDownloadService::new(state.db.clone(), state.tracked.clone());
    let client_repo = DownloadClientRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());

    let mut all_downloads = Vec::new();

    // Collect ALL tracked download IDs (regardless of status) so that
    // completed/imported downloads still suppress their untracked duplicates.
    // Without this, a status=4 (Imported) record drops out of the active set
    // and the torrent re-appears from the client as an untracked "ready to import".
    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let tracked_ids: HashSet<String> = match td_repo.get_all_download_ids().await {
        Ok(ids) => ids.into_iter().collect(),
        Err(e) => {
            tracing::warn!("Failed to load tracked download IDs: {}", e);
            HashSet::new()
        }
    };

    // Get tracked downloads with live status merged.
    // `get_queue()` returns a QueueResult containing both items and the raw
    // polled downloads from each client — reuse them below to avoid double-polling.
    let mut cached_client_downloads = std::collections::HashMap::new();

    match service.get_queue().await {
        Ok(QueueResult {
            items: queue_items,
            client_downloads,
        }) => {
            cached_client_downloads = client_downloads;
            for item in queue_items {
                // Skip downloads where the episode already has a file and
                // the download is waiting to be imported — already in the library
                if item.episode_has_file
                    && matches!(
                        item.tracked_download_state,
                        TrackedDownloadState::ImportPending | TrackedDownloadState::Imported
                    )
                {
                    continue;
                }
                all_downloads.push(queue_item_to_resource(&item));
            }
        }
        Err(e) => {
            tracing::warn!("Failed to get tracked downloads: {}", e);
        }
    }

    // Also include untracked downloads from clients (for backwards compatibility)
    if include_unknown {
        let clients = match client_repo.get_all().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to fetch download clients: {}", e);
                return all_downloads;
            }
        };

        let all_series = series_repo.get_all().await.unwrap_or_default();
        let movie_repo = MovieRepository::new(state.db.clone());
        let all_movies = movie_repo.get_all().await.unwrap_or_default();
        let mut id_counter = (all_downloads.len() as i64) + 10000;

        for db_client in clients.iter().filter(|c| c.enable) {
            // Parse the configured categories from client settings.
            let settings_json: serde_json::Value =
                serde_json::from_str(&db_client.settings).unwrap_or(serde_json::json!({}));
            let client_categories = ClientCategories::from_settings(&settings_json);

            // Reuse downloads already polled by get_queue() if available,
            // otherwise fall back to polling the client directly.
            let downloads = if let Some(cached) = cached_client_downloads.remove(&db_client.id) {
                cached
            } else {
                match create_client_from_model(db_client) {
                    Ok(client) => match client.get_downloads().await {
                        Ok(dl) => dl,
                        Err(e) => {
                            tracing::debug!(
                                "Failed to get downloads from {}: {}",
                                db_client.name,
                                e
                            );
                            continue;
                        }
                    },
                    Err(e) => {
                        tracing::debug!("Failed to create client for {}: {}", db_client.name, e);
                        continue;
                    }
                }
            };

            let protocol = if db_client.protocol == 1 {
                "usenet"
            } else {
                "torrent"
            };

            let total_dl_count = downloads.len();
            let mut skip_tracked = 0usize;
            let mut skip_category = 0usize;
            let mut skip_has_file = 0usize;
            let mut included = 0usize;

            for dl in downloads {
                if tracked_ids.contains(&dl.id) {
                    skip_tracked += 1;
                    continue;
                }

                // Skip downloads that don't match any configured category
                let dl_cat = dl.category.as_deref().unwrap_or("").to_lowercase();
                if !client_categories.all.is_empty()
                    && !client_categories.all.iter().any(|c| c == &dl_cat)
                {
                    skip_category += 1;
                    continue;
                }

                // Determine content type from download category
                let content_type = client_categories.content_type_for(&dl_cat);

                let status = match dl.status {
                    DownloadState::Queued => "queued",
                    DownloadState::Paused => "paused",
                    DownloadState::Downloading => "downloading",
                    DownloadState::Stalled => "stalled",
                    DownloadState::Seeding => "seeding",
                    DownloadState::Completed => "completed",
                    DownloadState::Failed => "failed",
                    DownloadState::Warning => "warning",
                };

                let tracked_state = match dl.status {
                    DownloadState::Queued => "downloading",
                    DownloadState::Downloading => "downloading",
                    DownloadState::Stalled => "downloading",
                    DownloadState::Paused => "paused",
                    DownloadState::Seeding => "importPending",
                    DownloadState::Completed => "importPending",
                    DownloadState::Failed => "downloadFailed",
                    DownloadState::Warning => "downloadWarning",
                };

                let timeleft = dl.eta.map(|seconds| {
                    let hours = seconds / 3600;
                    let minutes = (seconds % 3600) / 60;
                    let secs = seconds % 60;
                    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
                });

                let parsed = parse_title(&dl.name);
                let mut matched_series_id: Option<i64> = None;
                let mut matched_episode_id: Option<i64> = None;
                let mut quality_model = QualityModel {
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
                };

                if let Some(ref info) = parsed {
                    let q = &info.quality;
                    quality_model = QualityModel {
                        quality: QualityResource {
                            id: q.quality.weight(),
                            name: format!("{:?}", q.quality),
                            source: "unknown".to_string(),
                            resolution: q.quality.resolution_width(),
                        },
                        revision: RevisionResource {
                            version: q.revision.version,
                            real: q.revision.real,
                            is_repack: q.revision.is_repack,
                        },
                    };

                    // Use best_series_match (scored) instead of first-match to
                    // avoid partial substring matches picking the wrong series
                    if let Some(idx) = best_series_match(info, &all_series) {
                        let series = &all_series[idx];
                        matched_series_id = Some(series.id);

                        // Standard S01E02 matching
                        if let Some(season) = info.season_number {
                            if !info.episode_numbers.is_empty() {
                                let ep_num = info.episode_numbers[0];
                                if let Ok(Some(ep)) = episode_repo
                                    .get_by_series_season_episode(series.id, season, ep_num)
                                    .await
                                {
                                    matched_episode_id = Some(ep.id);
                                }
                            }
                        }

                        // Anime absolute episode matching (e.g. "- 23")
                        if matched_episode_id.is_none() && !info.absolute_episode_numbers.is_empty()
                        {
                            let abs_num = info.absolute_episode_numbers[0];
                            if let Ok(Some(ep)) = episode_repo
                                .get_by_series_and_absolute(series.id, abs_num)
                                .await
                            {
                                matched_episode_id = Some(ep.id);
                            }
                        }

                        // Bare episode number without season (e.g. "E10")
                        if matched_episode_id.is_none()
                            && info.season_number.is_none()
                            && !info.episode_numbers.is_empty()
                        {
                            let ep_num = info.episode_numbers[0];
                            // Try as absolute episode number first
                            if let Ok(Some(ep)) = episode_repo
                                .get_by_series_and_absolute(series.id, ep_num)
                                .await
                            {
                                matched_episode_id = Some(ep.id);
                            }
                        }
                    }
                }

                // Fallback: when parser can't extract structured data (e.g.
                // complete series packs with no S01E02 markers), try matching
                // the raw torrent name against known series titles directly.
                if matched_series_id.is_none() {
                    let name_normalized = normalize_title(&dl.name);
                    let mut best_match: Option<(i64, usize)> = None;

                    for series in &all_series {
                        let clean = normalize_title(&series.clean_title);
                        // Also try title without trailing year
                        let clean_no_year = clean
                            .trim_end()
                            .rsplit_once(' ')
                            .and_then(|(prefix, suffix)| {
                                if suffix.len() == 4 && suffix.chars().all(|c| c.is_ascii_digit()) {
                                    Some(prefix.to_string())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| clean.clone());

                        for candidate in [&clean, &clean_no_year] {
                            // Minimum length to avoid false positives (e.g. "V")
                            if candidate.len() >= 4 && name_normalized.contains(candidate.as_str())
                            {
                                if best_match.is_none()
                                    || candidate.len() > best_match.as_ref().unwrap().1
                                {
                                    best_match = Some((series.id, candidate.len()));
                                }
                            }
                        }
                    }

                    if let Some((series_id, _)) = best_match {
                        matched_series_id = Some(series_id);
                    }
                }

                // --- Movie match fallback (when no series matched) ---
                let mut matched_movie_id: Option<i64> = None;
                let mut matched_movie_title: Option<String> = None;
                let mut matched_movie_slug: Option<String> = None;
                if matched_series_id.is_none() {
                    let name_normalized = normalize_title(&dl.name);
                    let mut best_movie: Option<(i64, usize, &str, &str)> = None;

                    for movie in &all_movies {
                        let clean = normalize_title(&movie.clean_title);
                        if clean.len() >= 4 && name_normalized.contains(clean.as_str()) {
                            if best_movie.is_none() || clean.len() > best_movie.as_ref().unwrap().1
                            {
                                best_movie =
                                    Some((movie.id, clean.len(), &movie.title, &movie.title_slug));
                            }
                        }
                    }

                    if let Some((movie_id, _, title, slug)) = best_movie {
                        matched_movie_id = Some(movie_id);
                        matched_movie_title = Some(title.to_string());
                        matched_movie_slug = Some(slug.to_string());
                    }
                }

                // Check if matched media already has files in the library
                let mut media_has_file = false;

                if let Some(movie_id) = matched_movie_id {
                    // Movie: check if the movie record has_file
                    if let Some(movie) = all_movies.iter().find(|m| m.id == movie_id) {
                        media_has_file = movie.has_file;
                    }
                } else if let Some(ep_id) = matched_episode_id {
                    if let Ok(Some(ep)) = episode_repo.get_by_id(ep_id).await {
                        media_has_file = ep.has_file;
                    }
                } else if matched_series_id.is_some() {
                    // For season/series packs (no specific episode matched),
                    // check if all relevant episodes already have files.
                    // Use the full series episode list to avoid false positives
                    // when a multi-season pack (e.g. "S01-S02") only parses
                    // the first season number.
                    if let Some(ref info) = parsed {
                        if info.full_season {
                            if let Some(series_id) = matched_series_id {
                                let all_eps = episode_repo
                                    .get_by_series_id(series_id)
                                    .await
                                    .unwrap_or_default();
                                // Check all non-special episodes in the series.
                                // A season pack that matches a series should only
                                // be hidden if the entire series is complete.
                                let regular_eps: Vec<_> =
                                    all_eps.iter().filter(|e| e.season_number > 0).collect();
                                if !regular_eps.is_empty() && regular_eps.iter().all(|e| e.has_file)
                                {
                                    media_has_file = true;
                                }
                            }
                        }
                    }
                }

                // Skip completed/seeding downloads where media already imported
                if media_has_file
                    && matches!(dl.status, DownloadState::Completed | DownloadState::Seeding)
                {
                    skip_has_file += 1;
                    continue;
                }

                // Build series/episode from parsed info even when not matched in DB.
                // This gives the frontend a clean series name instead of the raw torrent title.
                let parsed_series = if matched_series_id.is_some() {
                    // Will be enriched later in list_queue()
                    None
                } else if let Some(ref info) = parsed {
                    if !info.series_title.is_empty() {
                        Some(QueueSeriesResource {
                            id: 0,
                            title: info.series_title.clone(),
                            title_slug: String::new(),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                let parsed_episode = if matched_episode_id.is_some() {
                    // Will be enriched later in list_queue()
                    None
                } else if let Some(ref info) = parsed {
                    if let Some(season) = info.season_number {
                        if !info.episode_numbers.is_empty() {
                            Some(QueueEpisodeResource {
                                id: 0,
                                season_number: season,
                                episode_number: info.episode_numbers[0],
                                title: String::new(),
                                air_date_utc: None,
                            })
                        } else {
                            None
                        }
                    } else if !info.absolute_episode_numbers.is_empty() {
                        // Anime absolute numbering (e.g. "- 23")
                        Some(QueueEpisodeResource {
                            id: 0,
                            season_number: 1,
                            episode_number: info.absolute_episode_numbers[0],
                            title: String::new(),
                            air_date_utc: None,
                        })
                    } else if !info.episode_numbers.is_empty() {
                        // Bare episode number without season (e.g. "E10")
                        Some(QueueEpisodeResource {
                            id: 0,
                            season_number: 0,
                            episode_number: info.episode_numbers[0],
                            title: String::new(),
                            air_date_utc: None,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Override content_type when we matched a movie
                let effective_content_type = if matched_movie_id.is_some() {
                    "movie".to_string()
                } else {
                    content_type.to_string()
                };

                let matched_movie_resource = matched_movie_id.map(|mid| QueueMovieResource {
                    id: mid,
                    title: matched_movie_title.clone().unwrap_or_default(),
                    title_slug: matched_movie_slug.clone().unwrap_or_default(),
                });

                all_downloads.push(QueueResource {
                    id: id_counter,
                    series_id: matched_series_id,
                    episode_id: matched_episode_id,
                    languages: vec![LanguageResource {
                        id: 1,
                        name: "English".to_string(),
                    }],
                    quality: quality_model,
                    custom_formats: vec![],
                    custom_format_score: 0,
                    size: dl.size as f64,
                    title: dl.name.clone(),
                    sizeleft: dl.size_left as f64,
                    timeleft,
                    estimated_completion_time: None,
                    added: None,
                    status: status.to_string(),
                    tracked_download_status: Some("ok".to_string()),
                    tracked_download_state: Some(tracked_state.to_string()),
                    status_messages: if dl.error_message.is_some() {
                        vec![StatusMessage {
                            title: "Error".to_string(),
                            messages: vec![dl.error_message.clone().unwrap_or_default()],
                        }]
                    } else {
                        vec![]
                    },
                    error_message: dl.error_message,
                    download_id: Some(dl.id),
                    protocol: protocol.to_string(),
                    download_client: Some(db_client.name.clone()),
                    download_client_has_post_import_category: false,
                    indexer: None,
                    output_path: dl.output_path,
                    episode_has_file: media_has_file,
                    content_type: effective_content_type,
                    movie_id: matched_movie_id,
                    artist_id: None,
                    audiobook_id: None,
                    album_id: None,
                    seeds: dl.seeds,
                    leechers: dl.leechers,
                    seed_count: dl.seed_count,
                    leech_count: dl.leech_count,
                    episode: parsed_episode,
                    series: parsed_series,
                    movie: matched_movie_resource,
                    artist: None,
                    audiobook: None,
                    import_progress: None,
                });

                id_counter += 1;
                included += 1;
            }

            tracing::debug!(
                "Queue filter for client '{}': total={}, skip_tracked={}, skip_category={}, skip_has_file={}, included={}",
                db_client.name, total_dl_count, skip_tracked, skip_category, skip_has_file, included
            );
        }
    }

    all_downloads
}
