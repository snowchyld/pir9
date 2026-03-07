//! Queue API endpoints (v5)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeFileRepository, EpisodeRepository, MovieFileRepository,
    MovieRepository, SeriesRepository, TrackedDownloadRepository,
};
use crate::core::download::clients::{create_client_from_model, DownloadState};
use crate::core::parser::{best_series_match, normalize_title, parse_title};
use crate::core::queue::{
    Protocol as QueueProtocol, QueueResult, QueueStatus, TrackedDownloadService,
    TrackedDownloadState, TrackedDownloadStatus,
};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct QueueListQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_unknown_series_items: Option<bool>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct QueueDetailsQuery {
    pub series_id: Option<i32>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RemoveFromQueueQuery {
    #[serde(default)]
    pub remove_from_client: bool,
    #[serde(default)]
    pub blocklist: bool,
    #[serde(default)]
    pub skip_redownload: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueResource {
    pub id: i64,
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub languages: Vec<LanguageResource>,
    pub quality: QualityModel,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
    pub size: f64,
    pub title: String,
    pub sizeleft: f64,
    pub timeleft: Option<String>,
    pub estimated_completion_time: Option<String>,
    pub added: Option<String>,
    pub status: String,
    pub tracked_download_status: Option<String>,
    pub tracked_download_state: Option<String>,
    pub status_messages: Vec<StatusMessage>,
    pub error_message: Option<String>,
    pub download_id: Option<String>,
    pub protocol: String,
    pub download_client: Option<String>,
    pub download_client_has_post_import_category: bool,
    pub indexer: Option<String>,
    pub output_path: Option<String>,
    pub episode_has_file: bool,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leechers: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<QueueEpisodeResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<QueueSeriesResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie: Option<QueueMovieResource>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueEpisodeResource {
    pub id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: String,
    pub air_date_utc: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueSeriesResource {
    pub id: i64,
    pub title: String,
    pub title_slug: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueMovieResource {
    pub id: i64,
    pub title: String,
    pub title_slug: String,
}

/// Per-content-type category mappings parsed from download client settings.
struct ClientCategories {
    movie: Vec<String>,
    anime: Vec<String>,
    /// Union of all categories — used for download filtering.
    all: Vec<String>,
}

impl ClientCategories {
    /// Parse category fields from download client settings JSON.
    /// Supports both the new split format (category/movieCategory/animeCategory)
    /// and the legacy comma-separated format in the `category` field.
    fn from_settings(settings: &serde_json::Value) -> Self {
        let get_cats = |key: &str| -> Vec<String> {
            settings
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| {
                    s.split(',')
                        .map(|c| c.trim().to_lowercase())
                        .filter(|c| !c.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };

        let has_new_format = settings.get("movieCategory").is_some();

        if has_new_format {
            // New split format: each key has its own categories
            let series = get_cats("category");
            let movie = get_cats("movieCategory");
            let anime = get_cats("animeCategory");

            let mut all = Vec::new();
            all.extend(series.iter().cloned());
            all.extend(movie.iter().cloned());
            all.extend(anime.iter().cloned());
            all.sort();
            all.dedup();

            Self { movie, anime, all }
        } else {
            // Legacy format: single comma-separated `category` field.
            // Infer content type from well-known category names.
            let all_cats = get_cats("category");
            let mut movie = Vec::new();
            let mut anime = Vec::new();

            for cat in &all_cats {
                match cat.as_str() {
                    "radarr" | "movies" | "movie" => movie.push(cat.clone()),
                    "anime" | "sonarr-anime" | "anime-sonarr" => anime.push(cat.clone()),
                    _ => {} // series (default)
                }
            }

            Self {
                movie,
                anime,
                all: all_cats,
            }
        }
    }

    /// Determine content type for a download based on its category.
    fn content_type_for(&self, category: &str) -> &'static str {
        let cat = category.to_lowercase();
        if self.movie.iter().any(|c| c == &cat) {
            return "movie";
        }
        if self.anime.iter().any(|c| c == &cat) {
            return "anime";
        }
        "series"
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatusMessage {
    pub title: String,
    pub messages: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResponse {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i64,
    pub records: Vec<QueueResource>,
    /// Number of previously imported downloads hidden from the queue.
    /// These have tracked_download records with status=4 (Imported) that
    /// suppress the torrent from reappearing. Clear them to reimport.
    pub hidden_imported_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatusResource {
    pub total_count: i32,
    pub count: i32,
    pub unknown_count: i32,
    pub errors: bool,
    pub warnings: bool,
    pub unknown_errors: bool,
    pub unknown_warnings: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueActionResponse {
    pub success: bool,
}

/// Convert a `QueueItem` from the service layer into a `QueueResource` for the API.
fn queue_item_to_resource(item: &crate::core::queue::QueueItem) -> QueueResource {
    let protocol = match item.protocol {
        QueueProtocol::Usenet => "usenet",
        QueueProtocol::Torrent => "torrent",
        QueueProtocol::Unknown => "unknown",
    };

    let status = match item.status {
        QueueStatus::Queued => "queued",
        QueueStatus::Paused => "paused",
        QueueStatus::Downloading => "downloading",
        QueueStatus::Completed => "completed",
        QueueStatus::Failed => "failed",
        QueueStatus::Warning => "warning",
        QueueStatus::Delay => "delay",
        QueueStatus::DownloadClientUnavailable => "downloadClientUnavailable",
        QueueStatus::Unknown => "unknown",
    };

    // Override status to "stalled" when the download is stalled (Warning from a Stalled state)
    // We detect this by checking if it's a warning with active seed/leech data showing 0 seeds
    let status = if status == "warning" && item.seeds == Some(0) && item.leechers == Some(0) {
        "stalled"
    } else {
        status
    };

    let tracked_state = match item.tracked_download_state {
        TrackedDownloadState::Downloading => "downloading",
        TrackedDownloadState::ImportBlocked => "importBlocked",
        TrackedDownloadState::ImportPending => "importPending",
        TrackedDownloadState::Importing => "importing",
        TrackedDownloadState::Imported => "imported",
        TrackedDownloadState::FailedPending => "failedPending",
        TrackedDownloadState::Failed => "failed",
        TrackedDownloadState::Ignored => "ignored",
    };

    let tracked_status = match item.tracked_download_status {
        TrackedDownloadStatus::Ok => "ok",
        TrackedDownloadStatus::Warning => "warning",
        TrackedDownloadStatus::Error => "error",
    };

    let quality_model = QualityModel {
        quality: QualityResource {
            id: item.quality.quality.weight(),
            name: format!("{:?}", item.quality.quality),
            source: "unknown".to_string(),
            resolution: item.quality.quality.resolution_width(),
        },
        revision: RevisionResource {
            version: item.quality.revision.version,
            real: item.quality.revision.real,
            is_repack: item.quality.revision.is_repack,
        },
    };

    let status_messages: Vec<StatusMessage> = item
        .status_messages
        .iter()
        .map(|sm| StatusMessage {
            title: sm.title.clone(),
            messages: sm.messages.clone(),
        })
        .collect();

    // Derive content type from movie_id presence
    let content_type = if item.movie_id > 0 { "movie" } else { "series" };

    QueueResource {
        id: item.id,
        series_id: if item.series_id > 0 {
            Some(item.series_id)
        } else {
            None
        },
        episode_id: if item.episode_id > 0 {
            Some(item.episode_id)
        } else {
            None
        },
        languages: vec![LanguageResource {
            id: 1,
            name: "English".to_string(),
        }],
        quality: quality_model,
        custom_formats: vec![],
        custom_format_score: 0,
        size: item.size as f64,
        title: item.title.clone(),
        sizeleft: item.sizeleft as f64,
        timeleft: item.timeleft.clone(),
        estimated_completion_time: item.estimated_completion_time.map(|t| t.to_rfc3339()),
        added: Some(item.added.to_rfc3339()),
        status: status.to_string(),
        tracked_download_status: Some(tracked_status.to_string()),
        tracked_download_state: Some(tracked_state.to_string()),
        status_messages,
        error_message: item.error_message.clone(),
        download_id: item.download_id.clone(),
        protocol: protocol.to_string(),
        download_client: Some(item.download_client.clone()),
        download_client_has_post_import_category: false,
        indexer: Some(item.indexer.clone()),
        output_path: item.output_path.clone(),
        episode_has_file: item.episode_has_file,
        content_type: content_type.to_string(),
        movie_id: if item.movie_id > 0 {
            Some(item.movie_id)
        } else {
            None
        },
        seeds: item.seeds,
        leechers: item.leechers,
        seed_count: item.seed_count,
        leech_count: item.leech_count,
        episode: None,
        series: None,
        movie: None,
    }
}

/// Fetch downloads from all enabled download clients.
/// Combines tracked downloads (from database) with untracked downloads (direct from clients).
async fn fetch_all_downloads(state: &AppState, include_unknown: bool) -> Vec<QueueResource> {
    let service = TrackedDownloadService::new(state.db.clone());
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

            for dl in downloads {
                if tracked_ids.contains(&dl.id) {
                    continue;
                }

                // Skip downloads that don't match any configured category
                let dl_cat = dl.category.as_deref().unwrap_or("").to_lowercase();
                if !client_categories.all.is_empty()
                    && !client_categories.all.iter().any(|c| c == &dl_cat)
                {
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
                    DownloadState::Queued => "importPending",
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
                        if matched_episode_id.is_none()
                            && !info.absolute_episode_numbers.is_empty()
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
                                if suffix.len() == 4
                                    && suffix.chars().all(|c| c.is_ascii_digit())
                                {
                                    Some(prefix.to_string())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| clean.clone());

                        for candidate in [&clean, &clean_no_year] {
                            // Minimum length to avoid false positives (e.g. "V")
                            if candidate.len() >= 4
                                && name_normalized.contains(candidate.as_str())
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
                            if best_movie.is_none()
                                || clean.len() > best_movie.as_ref().unwrap().1
                            {
                                best_movie = Some((
                                    movie.id,
                                    clean.len(),
                                    &movie.title,
                                    &movie.title_slug,
                                ));
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
                    // For season packs (no specific episode matched),
                    // check if all episodes in the season already have files
                    if let Some(ref info) = parsed {
                        if info.full_season {
                            if let (Some(season), Some(series_id)) =
                                (info.season_number, matched_series_id)
                            {
                                let season_eps = episode_repo
                                    .get_by_series_and_season(series_id, season)
                                    .await
                                    .unwrap_or_default();
                                if !season_eps.is_empty()
                                    && season_eps.iter().all(|e| e.has_file)
                                {
                                    media_has_file = true;
                                }
                            }
                        }
                    }
                }

                // Skip completed/seeding downloads where media already imported
                if media_has_file
                    && matches!(
                        dl.status,
                        DownloadState::Completed | DownloadState::Seeding
                    )
                {
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
                    seeds: dl.seeds,
                    leechers: dl.leechers,
                    seed_count: dl.seed_count,
                    leech_count: dl.leech_count,
                    episode: parsed_episode,
                    series: parsed_series,
                    movie: matched_movie_resource,
                });

                id_counter += 1;
            }
        }
    }

    all_downloads
}

async fn list_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    let include_unknown = params.include_unknown_series_items.unwrap_or(true);
    let include_episode = params.include_episode.unwrap_or(true);
    let include_series = params.include_series.unwrap_or(true);
    let mut all_downloads = fetch_all_downloads(&state, include_unknown).await;

    // Count hidden imported downloads (status=4) so the frontend can offer to clear them
    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let hidden_imported_count = td_repo.count_by_status(4).await.unwrap_or(0);

    // Enrich with episode/series/movie metadata
    if include_episode || include_series {
        let episode_repo = EpisodeRepository::new(state.db.clone());
        let series_repo = SeriesRepository::new(state.db.clone());
        let movie_repo = MovieRepository::new(state.db.clone());

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
    })
}

async fn get_queue_status(State(state): State<Arc<AppState>>) -> Json<QueueStatusResource> {
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

async fn get_queue_details(State(state): State<Arc<AppState>>) -> Json<Vec<QueueResource>> {
    let downloads = fetch_all_downloads(&state, true).await;
    Json(downloads)
}

async fn get_queue_item(
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

async fn remove_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<RemoveFromQueueQuery>,
) -> Json<QueueActionResponse> {
    let service = TrackedDownloadService::new(state.db.clone());

    if id < 10000 {
        if let Err(e) = service
            .remove(id, query.remove_from_client, query.blocklist)
            .await
        {
            tracing::warn!("Failed to remove tracked download {}: {}", id, e);
        } else {
            return Json(QueueActionResponse { success: true });
        }
    }

    // Fallback for untracked downloads
    let downloads = fetch_all_downloads(&state, true).await;
    if let Some(download) = downloads.iter().find(|d| d.id == id) {
        if let (Some(client_name), Some(download_id)) =
            (&download.download_client, &download.download_id)
        {
            let repo = DownloadClientRepository::new(state.db.clone());
            if let Ok(clients) = repo.get_all().await {
                for db_client in clients.iter().filter(|c| c.name == *client_name) {
                    if let Ok(client) = create_client_from_model(db_client) {
                        let _ = client.remove(download_id, query.remove_from_client).await;
                    }
                }
            }
        }
    }

    Json(QueueActionResponse { success: true })
}

async fn remove_from_queue(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RemoveFromQueueQuery>,
) -> Json<QueueActionResponse> {
    // Bulk remove all from queue
    let service = TrackedDownloadService::new(state.db.clone());
    let downloads = fetch_all_downloads(&state, true).await;

    for download in &downloads {
        if download.id < 10000 {
            let _ = service
                .remove(download.id, query.remove_from_client, query.blocklist)
                .await;
        }
    }

    Json(QueueActionResponse { success: true })
}

async fn grab_release(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<QueueActionResponse> {
    use crate::core::datastore::repositories::IndexerRepository;
    use crate::core::indexers::search::IndexerSearchService;
    use crate::core::indexers::SearchCriteria;

    let td_repo =
        crate::core::datastore::repositories::TrackedDownloadRepository::new(state.db.clone());

    // Look up the tracked download
    let tracked = match td_repo.get_by_id(id).await {
        Ok(Some(td)) => td,
        _ => return Json(QueueActionResponse { success: false }),
    };

    // Parse episode IDs from the tracked download
    let episode_ids: Vec<i64> = serde_json::from_str(&tracked.episode_ids).unwrap_or_default();
    if episode_ids.is_empty() {
        tracing::warn!("Re-grab: tracked download {} has no episode IDs", id);
        return Json(QueueActionResponse { success: false });
    }

    // Get episode and series info to build search criteria
    let episode_repo = EpisodeRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());

    let episode = match episode_repo.get_by_id(episode_ids[0]).await {
        Ok(Some(ep)) => ep,
        _ => return Json(QueueActionResponse { success: false }),
    };

    let series = match series_repo.get_by_id(tracked.series_id).await {
        Ok(Some(s)) => s,
        _ => return Json(QueueActionResponse { success: false }),
    };

    // Build search criteria from the tracked download's episode info
    let episode_numbers: Vec<i32> = {
        let mut nums = vec![episode.episode_number];
        for &ep_id in episode_ids.iter().skip(1) {
            if let Ok(Some(ep)) = episode_repo.get_by_id(ep_id).await {
                nums.push(ep.episode_number);
            }
        }
        nums
    };

    let criteria = SearchCriteria {
        series_id: series.tvdb_id,
        series_title: series.title.clone(),
        episode_id: Some(episode_ids[0]),
        season_number: Some(episode.season_number),
        episode_numbers,
        absolute_episode_numbers: vec![],
        special: episode.season_number == 0,
    };

    // Search indexers for matching releases
    let indexer_repo = IndexerRepository::new(state.db.clone());
    let indexers = match indexer_repo.get_all().await {
        Ok(i) => i,
        Err(_) => return Json(QueueActionResponse { success: false }),
    };

    let search_service = IndexerSearchService::new(indexers);
    let releases = match search_service.search(&criteria).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Re-grab search failed for '{}': {}", tracked.title, e);
            return Json(QueueActionResponse { success: false });
        }
    };

    if releases.is_empty() {
        tracing::warn!("Re-grab: no releases found for '{}'", tracked.title);
        return Json(QueueActionResponse { success: false });
    }

    // Remove the old tracked download first
    let service = TrackedDownloadService::new(state.db.clone());
    let _ = service.remove(id, false, false).await;

    // Grab the best release (first in quality-sorted list)
    let best = &releases[0];
    match service.grab_release(best, episode_ids, tracked.movie_id).await {
        Ok(new_id) => {
            tracing::info!(
                "Re-grab succeeded: {} → tracked download {}",
                tracked.title,
                new_id
            );
            Json(QueueActionResponse { success: true })
        }
        Err(e) => {
            tracing::warn!("Re-grab failed for '{}': {}", tracked.title, e);
            Json(QueueActionResponse { success: false })
        }
    }
}

/// POST /api/v5/queue/{id}/import
/// Import a completed download into the library
async fn import_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    body: axum::body::Bytes,
) -> Json<QueueActionResponse> {
    use crate::core::datastore::repositories::TrackedDownloadRepository;
    use crate::core::download::import::ImportService;

    // Parse optional overrides from request body
    let overrides: std::collections::HashMap<String, EpisodeOverride> = if body.is_empty() {
        std::collections::HashMap::new()
    } else {
        match serde_json::from_slice::<ImportQueueBody>(&body) {
            Ok(b) => b.overrides.unwrap_or_default(),
            Err(_) => std::collections::HashMap::new(),
        }
    };

    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let client_repo = DownloadClientRepository::new(state.db.clone());
    let import_service = ImportService::new(state.db.clone(), state.config.read().media.clone());

    // Find the download — either tracked (id < 10000) or untracked
    let (download_id, download_client_id, title, tracked_movie_id) = if id < 10000 {
        // Tracked download — look up from DB
        match td_repo.get_by_id(id).await {
            Ok(Some(td)) => (td.download_id, td.download_client_id, td.title, td.movie_id),
            _ => {
                tracing::warn!("Import: tracked download {} not found", id);
                return Json(QueueActionResponse { success: false });
            }
        }
    } else {
        // Untracked download — find from queue data
        let downloads = fetch_all_downloads(&state, true).await;
        match downloads.into_iter().find(|d| d.id == id) {
            Some(dl) => {
                let dl_id = match dl.download_id {
                    Some(id) => id,
                    None => return Json(QueueActionResponse { success: false }),
                };
                let client_name = dl.download_client.unwrap_or_default();
                // Look up client ID by name
                let clients = client_repo.get_all().await.unwrap_or_default();
                let client_id = clients
                    .iter()
                    .find(|c| c.name == client_name)
                    .map(|c| c.id)
                    .unwrap_or(0);
                (dl_id, client_id, dl.title, dl.movie_id)
            }
            None => {
                tracing::warn!("Import: queue item {} not found", id);
                return Json(QueueActionResponse { success: false });
            }
        }
    };

    // Get live download status from the client (for the real content path)
    let client_model = match client_repo.get_by_id(download_client_id).await {
        Ok(Some(c)) => c,
        _ => {
            tracing::warn!("Import: download client {} not found", download_client_id);
            return Json(QueueActionResponse { success: false });
        }
    };

    let client = match create_client_from_model(&client_model) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Import: failed to create client: {}", e);
            return Json(QueueActionResponse { success: false });
        }
    };

    let live_status = match client.get_download(&download_id).await {
        Ok(Some(dl)) => dl,
        Ok(None) => {
            tracing::warn!("Import: download {} not found in client", download_id);
            return Json(QueueActionResponse { success: false });
        }
        Err(e) => {
            tracing::warn!("Import: failed to query client: {}", e);
            return Json(QueueActionResponse { success: false });
        }
    };

    let raw_output_path = match live_status.output_path {
        Some(ref p) => p.clone(),
        None => {
            tracing::warn!("Import: no output path for download {}", download_id);
            return Json(QueueActionResponse { success: false });
        }
    };

    // Apply remote path mappings (translate client paths to local paths)
    let output_path = {
        use crate::core::datastore::repositories::RemotePathMappingRepository;
        let mapping_repo = RemotePathMappingRepository::new(state.db.clone());
        let mut mapped = raw_output_path.clone();
        let mut did_map = false;
        if let Ok(mappings) = mapping_repo.get_all().await {
            for m in &mappings {
                if mapped.starts_with(&m.remote_path) {
                    mapped = mapped.replacen(&m.remote_path, &m.local_path, 1);
                    tracing::debug!("Import: mapped path '{}' -> '{}'", raw_output_path, mapped);
                    did_map = true;
                    break;
                }
            }
        }
        if !did_map {
            tracing::warn!(
                "Import: no remote path mapping matched '{}' — check Settings > Download Clients > Remote Path Mappings",
                raw_output_path
            );
        }
        mapped
    };

    // Movie import: if this download is matched to a movie, use the movie import flow
    if let Some(movie_id) = tracked_movie_id {
        // Dispatch to worker when Redis is available (fast local disk access)
        if let Some(ref hybrid_bus) = state.hybrid_event_bus {
            if hybrid_bus.is_redis_enabled() {
                let movie_repo = MovieRepository::new(state.db.clone());
                let movie_title = match movie_repo.get_by_id(movie_id).await {
                    Ok(Some(m)) => m.title,
                    _ => title.clone(),
                };

                let (job_id, message) =
                    crate::core::scanner::create_movie_scan_request(
                        vec![movie_id],
                        vec![output_path.clone()],
                        std::collections::HashMap::new(),
                    );
                if let Some(consumer) = state.scan_result_consumer.get() {
                    consumer
                        .register_job(
                            &job_id,
                            crate::core::messaging::ScanType::RescanMovie,
                            vec![movie_id],
                        )
                        .await;
                }
                hybrid_bus.publish(message).await;
                tracing::info!(
                    "Queue movie import: dispatched '{}' to worker (job_id={})",
                    movie_title,
                    job_id
                );
                return Json(QueueActionResponse { success: true });
            }
        }

        // Local fallback: scan + probe + hash over NFS
        let movie_repo = MovieRepository::new(state.db.clone());
        let movie_file_repo = MovieFileRepository::new(state.db.clone());

        let movie = match movie_repo.get_by_id(movie_id).await {
            Ok(Some(m)) => m,
            _ => {
                tracing::warn!(
                    "Import: movie {} not found for download {}",
                    movie_id,
                    download_id
                );
                return Json(QueueActionResponse { success: false });
            }
        };

        let db = state.db.clone();
        let movie_title = movie.title.clone();
        let dl_title = title.clone();
        tokio::spawn(async move {
            // scan_movie_folder finds the largest video file in the output path
            if let Some(mut movie_file) = super::movies::scan_movie_folder(&output_path, movie_id) {
                // Check if this file is already tracked (prevent duplicate imports)
                if let Ok(existing_files) = movie_file_repo.get_by_movie_id(movie_id).await {
                    if existing_files.iter().any(|f| f.path == movie_file.path) {
                        tracing::info!(
                            "Movie import: file already tracked for '{}': {}",
                            movie_title, movie_file.path
                        );
                        return;
                    }
                }

                // Media analysis: probe with FFmpeg + BLAKE3 hash (same as folder import)
                let file_path = std::path::Path::new(&movie_file.path);
                if let Ok(info) = crate::core::mediafiles::MediaAnalyzer::analyze(file_path).await {
                    movie_file.media_info = serde_json::to_string(&info).ok();
                    let quality =
                        crate::core::mediafiles::derive_quality_from_media(&info, &movie_file.path);
                    movie_file.quality = serde_json::to_string(&quality)
                        .unwrap_or_else(|_| movie_file.quality.clone());
                }
                movie_file.file_hash = crate::core::mediafiles::compute_file_hash(file_path)
                    .await
                    .ok();

                match movie_file_repo.insert(&movie_file).await {
                    Ok(file_id) => {
                        // Update movie to have a file
                        let pool = db.pool();
                        let _ = sqlx::query(
                            "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2",
                        )
                        .bind(file_id)
                        .bind(movie_id)
                        .execute(pool)
                        .await;

                        tracing::info!(
                            "Movie imported: '{}' → movie {} (file: {})",
                            dl_title,
                            movie_title,
                            movie_file.path,
                        );

                        // Delete tracked download — the import is complete and the
                        // record no longer needs to linger. Leaving it as status=4
                        // caused the torrent to re-appear as "untracked" because
                        // get_all_active() (status < 4) excluded it from suppression.
                        if id < 10000 {
                            let td_repo = TrackedDownloadRepository::new(db);
                            let _ = td_repo.delete(id).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Import: failed to insert movie file for {}: {}",
                            movie_title,
                            e
                        );
                    }
                }
            } else {
                tracing::warn!(
                    "Import: no video file found in '{}' for movie {}",
                    output_path,
                    movie_title
                );
            }
        });

        return Json(QueueActionResponse { success: true });
    }

    // Build a PendingImport using the import service's matching logic
    let mut parsed = crate::core::parser::parse_title(&title);
    let mut series = None;
    let mut episodes = Vec::new();

    if let Some(ref info) = parsed {
        if let Ok(s) = import_service.match_series(info).await {
            if let Some(ref matched) = s {
                if let Ok(eps) = import_service.match_episodes(matched, info).await {
                    episodes = eps;
                }
            }
            series = s;
        }
    }

    // Fallback: when parser can't extract structured data (complete series
    // packs without S01E02 markers), or when it extracts quality info but no
    // season/episode numbers (so match_episodes returns empty), match the raw
    // title against known series and treat it as a multi-season pack so
    // per-file parsing handles individual episode assignment.
    if series.is_none() || episodes.is_empty() {
        let series_repo = SeriesRepository::new(state.db.clone());
        let episode_repo = EpisodeRepository::new(state.db.clone());

        // If series was already matched via primary path, use it directly
        // instead of re-matching against all series.
        if let Some(ref matched) = series {
            tracing::info!(
                "Import fallback: series '{}' matched but no episodes resolved, loading all episodes for pack import",
                matched.title
            );
            if let Ok(eps) = episode_repo.get_by_series_id(matched.id).await {
                episodes = eps;
            }
            parsed = Some(crate::core::parser::ParsedEpisodeInfo {
                series_title: matched.clean_title.clone(),
                full_season: true,
                is_multi_season: true,
                raw_title: title.clone(),
                ..Default::default()
            });
        } else if let Ok(all_series) = series_repo.get_all().await {
            let name_normalized = normalize_title(&title);
            let mut best_match: Option<(usize, usize)> = None;

            for (idx, s) in all_series.iter().enumerate() {
                let clean = normalize_title(&s.clean_title);
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
                    if candidate.len() >= 4 && name_normalized.contains(candidate.as_str()) {
                        if best_match.is_none() || candidate.len() > best_match.as_ref().unwrap().1
                        {
                            best_match = Some((idx, candidate.len()));
                        }
                    }
                }
            }

            if let Some((idx, _)) = best_match {
                let matched = all_series.into_iter().nth(idx).unwrap();
                tracing::info!(
                    "Import fallback: matched '{}' to series '{}'",
                    title,
                    matched.title
                );
                if let Ok(eps) = episode_repo.get_by_series_id(matched.id).await {
                    episodes = eps;
                }
                parsed = Some(crate::core::parser::ParsedEpisodeInfo {
                    series_title: matched.clean_title.clone(),
                    full_season: true,
                    is_multi_season: true,
                    raw_title: title.clone(),
                    ..Default::default()
                });
                series = Some(matched);
            }
        }
    }

    // Apply manual episode overrides from the UI
    if !overrides.is_empty() {
        if let Some(ref matched_series) = series {
            let episode_repo = EpisodeRepository::new(state.db.clone());
            if let Ok(all_eps) = episode_repo.get_by_series_id(matched_series.id).await {
                for ov in overrides.values() {
                    if let Some(ep) = all_eps.iter().find(|e| {
                        e.season_number == ov.season_number
                            && e.episode_number == ov.episode_number
                    }) {
                        if !episodes.iter().any(|e| e.id == ep.id) {
                            episodes.push(ep.clone());
                        }
                    }
                }
                // Ensure we have full_season/is_multi_season set for pack-style import
                if parsed.is_none() || episodes.len() > 1 {
                    parsed = Some(crate::core::parser::ParsedEpisodeInfo {
                        series_title: matched_series.clean_title.clone(),
                        full_season: true,
                        is_multi_season: true,
                        raw_title: title.clone(),
                        ..Default::default()
                    });
                }
            }
        }
        tracing::info!(
            "Import: {} manual override(s) applied for '{}'",
            overrides.len(),
            title
        );
    }

    let pending = crate::core::download::import::PendingImport {
        download_id: download_id.clone(),
        download_client_id,
        download_client_name: client_model.name.clone(),
        title: title.clone(),
        output_path: std::path::PathBuf::from(&output_path),
        parsed_info: parsed,
        series,
        episodes,
        overrides: overrides
            .iter()
            .map(|(k, v)| (k.clone(), (v.season_number, v.episode_number)))
            .collect(),
    };

    // Dispatch to Redis worker when available — worker has fast local disk access
    // for FFmpeg probing + BLAKE3 hashing instead of going over NFS
    if let Some(ref hybrid_bus) = state.hybrid_event_bus {
        if hybrid_bus.is_redis_enabled() {
            if let Some(consumer) = state.scan_result_consumer.get() {
                let job_id = uuid::Uuid::new_v4().to_string();

                let import_info = crate::core::scanner::DownloadImportInfo {
                    download_id: pending.download_id.clone(),
                    download_client_id: pending.download_client_id,
                    download_client_name: pending.download_client_name.clone(),
                    title: pending.title.clone(),
                    output_path: pending.output_path.clone(),
                    parsed_info: pending.parsed_info.clone(),
                    series: pending.series.clone(),
                    episodes: pending.episodes.clone(),
                    overrides: overrides
                        .iter()
                        .map(|(k, v)| (k.clone(), (v.season_number, v.episode_number)))
                        .collect(),
                };

                consumer
                    .register_download_import(&job_id, vec![import_info])
                    .await;
                consumer
                    .register_job(
                        &job_id,
                        crate::core::messaging::ScanType::DownloadedEpisodesScan,
                        vec![0],
                    )
                    .await;

                let message = crate::core::messaging::Message::ScanRequest {
                    job_id: job_id.clone(),
                    scan_type: crate::core::messaging::ScanType::DownloadedEpisodesScan,
                    series_ids: vec![0],
                    paths: vec![output_path.clone()],
                    known_files: std::collections::HashMap::new(),
                };
                hybrid_bus.publish(message).await;

                tracing::info!(
                    "Queue series import: dispatched '{}' to worker (job_id={})",
                    pending.title,
                    job_id,
                );
                return Json(QueueActionResponse { success: true });
            }
        }
    }

    // Fallback: run the import locally (no Redis worker available).
    // Season/multi-season packs can take minutes (FFmpeg probing + hashing per file).
    let db = state.db.clone();
    tokio::spawn(async move {
        match import_service.import(&pending).await {
            Ok(result) if result.success => {
                tracing::info!(
                    "Imported '{}': {} files, {} episodes",
                    pending.title,
                    result.files_imported,
                    result.episode_ids.len()
                );

                // Delete tracked download — import is complete, no need to
                // keep the record (status=4 records caused ghost duplicates).
                if id < 10000 {
                    let td_repo = TrackedDownloadRepository::new(db);
                    let _ = td_repo.delete(id).await;
                }
            }
            Ok(result) => {
                let msg = result
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string());
                tracing::warn!("Import failed for '{}': {}", pending.title, msg);
            }
            Err(e) => {
                tracing::warn!("Import error for '{}': {}", pending.title, e);
            }
        }
    });

    Json(QueueActionResponse { success: true })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueFileResource {
    pub name: String,
    pub size: i64,
}

/// GET /api/v5/queue/{id}/files
/// Returns the file list for a download (from the download client)
async fn get_queue_files(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<QueueFileResource>>, StatusCode> {
    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let client_repo = DownloadClientRepository::new(state.db.clone());

    // Resolve download_id and client for this queue item
    let (download_id, client_id) = if id < 10000 {
        // Tracked download
        match td_repo.get_by_id(id).await {
            Ok(Some(td)) => (td.download_id, td.download_client_id),
            _ => return Err(StatusCode::NOT_FOUND),
        }
    } else {
        // Untracked download — find from live queue data
        let downloads = fetch_all_downloads(&state, true).await;
        match downloads.iter().find(|d| d.id == id) {
            Some(dl) => {
                let dl_id = dl.download_id.clone().unwrap_or_default();
                let client_name = dl.download_client.clone().unwrap_or_default();
                let clients = client_repo.get_all().await.unwrap_or_default();
                match clients.iter().find(|c| c.name == client_name) {
                    Some(c) => (dl_id, c.id),
                    None => return Err(StatusCode::NOT_FOUND),
                }
            }
            None => return Err(StatusCode::NOT_FOUND),
        }
    };

    // Get client and fetch files
    let client_model = match client_repo.get_by_id(client_id).await {
        Ok(Some(c)) => c,
        _ => return Err(StatusCode::NOT_FOUND),
    };

    let client = match create_client_from_model(&client_model) {
        Ok(c) => c,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    match client.get_files(&download_id).await {
        Ok(files) => Ok(Json(
            files
                .into_iter()
                .map(|f| QueueFileResource {
                    name: f.name,
                    size: f.size,
                })
                .collect(),
        )),
        Err(e) => {
            tracing::warn!("Failed to get files for download {}: {}", download_id, e);
            // Return empty list rather than error — usenet clients don't support file listing
            Ok(Json(vec![]))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMatchRequest {
    /// Series ID (for series/anime match)
    pub series_id: Option<i64>,
    /// Episode IDs (for series/anime match)
    pub episode_ids: Option<Vec<i64>>,
    /// Movie ID (for movie match — mutually exclusive with series_id)
    pub movie_id: Option<i64>,
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
async fn update_match(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateMatchRequest>,
) -> Result<Json<QueueActionResponse>, StatusCode> {
    let td_repo = TrackedDownloadRepository::new(state.db.clone());

    // Determine if this is a movie match or series match
    let is_movie_match = body.movie_id.is_some();

    if is_movie_match {
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
        let series_id = match body.series_id {
            Some(sid) => sid,
            None => return Err(StatusCode::BAD_REQUEST),
        };
        let episode_ids = body.episode_ids.unwrap_or_default();

        let series_repo = SeriesRepository::new(state.db.clone());
        let episode_repo = EpisodeRepository::new(state.db.clone());

        // Validate series exists
        match series_repo.get_by_id(series_id).await {
            Ok(Some(_)) => {}
            _ => return Err(StatusCode::NOT_FOUND),
        }

        // Validate episode IDs exist
        for &ep_id in &episode_ids {
            match episode_repo.get_by_id(ep_id).await {
                Ok(Some(_)) => {}
                _ => return Err(StatusCode::NOT_FOUND),
            }
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

// ─── Import Preview ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewResponse {
    pub id: i64,
    pub title: String,
    pub content_type: String,
    pub series: Option<ImportPreviewSeries>,
    pub movie: Option<ImportPreviewMovie>,
    pub output_path: String,
    pub files: Vec<ImportPreviewFile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<ImportPreviewEpisode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewSeries {
    pub id: i64,
    pub title: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewMovie {
    pub id: i64,
    pub title: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewFile {
    pub source_file: String,
    pub source_size: i64,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub episode_title: Option<String>,
    pub destination_path: Option<String>,
    pub matched: bool,
    pub existing_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_file_size: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewEpisode {
    pub id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: String,
    pub has_file: bool,
    pub file_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportQueueBody {
    pub overrides: Option<std::collections::HashMap<String, EpisodeOverride>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeOverride {
    pub season_number: i32,
    pub episode_number: i32,
}

/// GET /api/v5/queue/{id}/import-preview
/// Preview what an import will do before committing
async fn get_import_preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ImportPreviewResponse>, StatusCode> {
    use crate::core::datastore::repositories::{
        EpisodeRepository, RemotePathMappingRepository, TrackedDownloadRepository,
    };
    use crate::core::download::import::compute_destination_path;
    use crate::core::parser::parse_title;
    use crate::core::scanner::{is_video_file, parse_episodes_from_filename};

    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let client_repo = DownloadClientRepository::new(state.db.clone());

    // Resolve download: tracked (id < 10000) or untracked
    let (download_id, download_client_id, title, tracked_series_id, tracked_movie_id, stored_output_path) =
        if id < 10000 {
            match td_repo.get_by_id(id).await {
                Ok(Some(td)) => (
                    td.download_id,
                    td.download_client_id,
                    td.title,
                    if td.series_id > 0 {
                        Some(td.series_id)
                    } else {
                        None
                    },
                    td.movie_id,
                    td.output_path,
                ),
                _ => return Err(StatusCode::NOT_FOUND),
            }
        } else {
            let downloads = fetch_all_downloads(&state, true).await;
            match downloads.into_iter().find(|d| d.id == id) {
                Some(dl) => {
                    let dl_id = dl.download_id.ok_or(StatusCode::NOT_FOUND)?;
                    let client_name = dl.download_client.unwrap_or_default();
                    let clients = client_repo.get_all().await.unwrap_or_default();
                    let client_id = clients
                        .iter()
                        .find(|c| c.name == client_name)
                        .map(|c| c.id)
                        .ok_or(StatusCode::NOT_FOUND)?;
                    (
                        dl_id,
                        client_id,
                        dl.title,
                        dl.series_id.filter(|&sid| sid > 0),
                        dl.movie_id,
                        dl.output_path,
                    )
                }
                None => return Err(StatusCode::NOT_FOUND),
            }
        };

    // Try to get live status from client; fall back to stored output_path if gone
    let client_model = client_repo
        .get_by_id(download_client_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let client =
        create_client_from_model(&client_model).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let live_status = client
        .get_download(&download_id)
        .await
        .unwrap_or(None);

    let raw_output_path = live_status
        .as_ref()
        .and_then(|s| s.output_path.clone())
        .or(stored_output_path)
        .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;

    // Apply remote path mappings
    let output_path = {
        let mapping_repo = RemotePathMappingRepository::new(state.db.clone());
        let mut mapped = raw_output_path.clone();
        if let Ok(mappings) = mapping_repo.get_all().await {
            for m in &mappings {
                if mapped.starts_with(&m.remote_path) {
                    mapped = mapped.replacen(&m.remote_path, &m.local_path, 1);
                    break;
                }
            }
        }
        mapped
    };

    // Get file list: try download client first, fall back to scanning the output path
    let dl_files = if live_status.is_some() {
        client.get_files(&download_id).await.unwrap_or_default()
    } else {
        // Client doesn't have this download anymore — scan the filesystem
        use crate::core::download::clients::DownloadFile;
        let scan_path = output_path.clone();
        tokio::task::spawn_blocking(move || {
            let path = std::path::Path::new(&scan_path);
            if !path.exists() {
                return vec![];
            }
            if path.is_file() {
                let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
                return vec![DownloadFile {
                    name: path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    size,
                }];
            }
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let size = std::fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0);
                        files.push(DownloadFile {
                            name: p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            size,
                        });
                    } else if p.is_dir() {
                        // One level of subdirectory
                        if let Ok(sub_entries) = std::fs::read_dir(&p) {
                            let dir_name = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            for sub_entry in sub_entries.flatten() {
                                let sp = sub_entry.path();
                                if sp.is_file() {
                                    let size = std::fs::metadata(&sp)
                                        .map(|m| m.len() as i64)
                                        .unwrap_or(0);
                                    let name = format!(
                                        "{}/{}",
                                        dir_name,
                                        sp.file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_default()
                                    );
                                    files.push(DownloadFile { name, size });
                                }
                            }
                        }
                    }
                }
            }
            files
        })
        .await
        .unwrap_or_default()
    };

    let media_config = state.config.read().media.clone();

    // ── Movie preview ──
    if let Some(movie_id) = tracked_movie_id {
        let movie_repo = MovieRepository::new(state.db.clone());
        let movie = movie_repo
            .get_by_id(movie_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        let files: Vec<ImportPreviewFile> = dl_files
            .iter()
            .map(|f| {
                let filename = f.name.split('/').last().unwrap_or(&f.name);
                let is_video =
                    is_video_file(std::path::Path::new(filename));
                ImportPreviewFile {
                    source_file: f.name.clone(),
                    source_size: f.size,
                    season_number: None,
                    episode_number: None,
                    episode_title: None,
                    destination_path: if is_video {
                        Some(movie.path.clone())
                    } else {
                        None
                    },
                    matched: is_video,
                    existing_file: movie.has_file,
                    existing_file_size: None,
                }
            })
            .collect();

        return Ok(Json(ImportPreviewResponse {
            id,
            title,
            content_type: "movie".to_string(),
            series: None,
            movie: Some(ImportPreviewMovie {
                id: movie.id,
                title: movie.title,
                path: movie.path,
            }),
            output_path,
            files,
            episodes: vec![],
        }));
    }

    // ── Series preview ──
    let series_repo = SeriesRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());

    // Resolve series: tracked series_id or parse from title
    let series = if let Some(sid) = tracked_series_id {
        series_repo
            .get_by_id(sid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?
    } else {
        // Try to match from release title
        let parsed = parse_title(&title).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
        crate::core::download::import::match_series_standalone(&state.db, &parsed)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?
    };

    // Load all episodes for this series
    let all_episodes = episode_repo
        .get_by_series_id(series.id)
        .await
        .unwrap_or_default();

    // Episodes that already have files (use has_file flag from episode model)
    let episodes_with_files: HashSet<i64> = all_episodes
        .iter()
        .filter(|e| e.has_file)
        .map(|e| e.id)
        .collect();

    // Load episode file sizes for existing file comparison
    let episode_file_repo = EpisodeFileRepository::new(state.db.clone());
    let file_size_map: std::collections::HashMap<i64, i64> = episode_file_repo
        .get_by_series_id(series.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.id, f.size))
        .collect();

    // Parse the release title for quality/group info
    let parsed_info = parse_title(&title).unwrap_or_default();

    // Build preview for each file
    let mut preview_files = Vec::new();
    for f in &dl_files {
        let filename = f.name.split('/').last().unwrap_or(&f.name);
        let is_video = is_video_file(std::path::Path::new(filename));

        if !is_video {
            preview_files.push(ImportPreviewFile {
                source_file: f.name.clone(),
                source_size: f.size,
                season_number: None,
                episode_number: None,
                episode_title: None,
                destination_path: None,
                matched: false,
                existing_file: false,
                existing_file_size: None,
            });
            continue;
        }

        let parsed_eps = parse_episodes_from_filename(filename);

        if parsed_eps.is_empty() {
            // Try special matching as fallback
            let specials: Vec<(i32, &str)> = all_episodes
                .iter()
                .filter(|e| e.season_number == 0)
                .map(|e| (e.episode_number, e.title.as_str()))
                .collect();
            if let Some((season, ep)) =
                crate::core::scanner::match_special_by_title(filename, &series.title, &specials)
            {
                let matched_ep = all_episodes
                    .iter()
                    .find(|e| e.season_number == season && e.episode_number == ep);
                let has_file = matched_ep
                    .map(|e| episodes_with_files.contains(&e.id))
                    .unwrap_or(false);
                let dest = matched_ep.map(|e| {
                    compute_destination_path(
                        &media_config,
                        &series,
                        season,
                        filename,
                        &[e.clone()],
                        &parsed_info,
                    )
                    .to_string_lossy()
                    .to_string()
                });
                let existing_size = matched_ep
                    .and_then(|e| e.episode_file_id)
                    .and_then(|fid| file_size_map.get(&fid).copied());
                preview_files.push(ImportPreviewFile {
                    source_file: f.name.clone(),
                    source_size: f.size,
                    season_number: Some(season),
                    episode_number: Some(ep),
                    episode_title: matched_ep.map(|e| e.title.clone()),
                    destination_path: dest,
                    matched: true,
                    existing_file: has_file,
                    existing_file_size: existing_size,
                });
            } else {
                preview_files.push(ImportPreviewFile {
                    source_file: f.name.clone(),
                    source_size: f.size,
                    season_number: None,
                    episode_number: None,
                    episode_title: None,
                    destination_path: None,
                    matched: false,
                    existing_file: false,
                    existing_file_size: None,
                });
            }
            continue;
        }

        // For each parsed episode (handles multi-episode files)
        let (season, first_ep) = parsed_eps[0];
        let matched_episodes: Vec<_> = parsed_eps
            .iter()
            .filter_map(|&(s, e)| {
                all_episodes
                    .iter()
                    .find(|ep| ep.season_number == s && ep.episode_number == e)
                    .cloned()
            })
            .collect();

        let has_file = matched_episodes
            .iter()
            .any(|e| episodes_with_files.contains(&e.id));

        let dest = if !matched_episodes.is_empty() {
            Some(
                compute_destination_path(
                    &media_config,
                    &series,
                    season,
                    filename,
                    &matched_episodes,
                    &parsed_info,
                )
                .to_string_lossy()
                .to_string(),
            )
        } else {
            None
        };

        let existing_size = matched_episodes
            .first()
            .and_then(|e| e.episode_file_id)
            .and_then(|fid| file_size_map.get(&fid).copied());
        preview_files.push(ImportPreviewFile {
            source_file: f.name.clone(),
            source_size: f.size,
            season_number: Some(season),
            episode_number: Some(first_ep),
            episode_title: matched_episodes.first().map(|e| e.title.clone()),
            destination_path: dest,
            matched: !matched_episodes.is_empty(),
            existing_file: has_file,
            existing_file_size: existing_size,
        });
    }

    // Build episode list for manual matching dropdowns
    let preview_episodes: Vec<ImportPreviewEpisode> = all_episodes
        .iter()
        .map(|e| ImportPreviewEpisode {
            id: e.id,
            season_number: e.season_number,
            episode_number: e.episode_number,
            title: e.title.clone(),
            has_file: e.has_file,
            file_size: e.episode_file_id.and_then(|fid| file_size_map.get(&fid).copied()),
        })
        .collect();

    Ok(Json(ImportPreviewResponse {
        id,
        title,
        content_type: if series.series_type == 2 {
            "anime".to_string()
        } else {
            "series".to_string()
        },
        series: Some(ImportPreviewSeries {
            id: series.id,
            title: series.title,
            path: series.path,
        }),
        movie: None,
        output_path,
        files: preview_files,
        episodes: preview_episodes,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedDeleteQuery {
    /// Filter by tracked download status (e.g. 4 = Imported)
    pub status: Option<i32>,
}

/// Clear tracked download records, optionally filtered by status.
/// This allows previously imported torrents to reappear in the queue for reimport.
async fn clear_tracked_downloads(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrackedDeleteQuery>,
) -> Json<QueueActionResponse> {
    let td_repo = TrackedDownloadRepository::new(state.db.clone());

    let deleted = if let Some(status) = query.status {
        td_repo.delete_all_by_status(status).await.unwrap_or(0)
    } else {
        // Without a status filter, only clear imported (status=4) as a safe default
        td_repo.delete_all_by_status(4).await.unwrap_or(0)
    };

    tracing::info!("Cleared {} tracked download records (status filter: {:?})", deleted, query.status);
    Json(QueueActionResponse { success: true })
}

/// Delete a single tracked download record by ID.
async fn delete_tracked_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<QueueActionResponse> {
    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    match td_repo.delete(id).await {
        Ok(()) => {
            tracing::info!("Deleted tracked download record: {}", id);
            Json(QueueActionResponse { success: true })
        }
        Err(e) => {
            tracing::warn!("Failed to delete tracked download {}: {}", id, e);
            Json(QueueActionResponse { success: false })
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_queue).delete(remove_from_queue))
        .route("/status", get(get_queue_status))
        .route("/details", get(get_queue_details))
        .route("/tracked", axum::routing::delete(clear_tracked_downloads))
        .route("/tracked/{id}", axum::routing::delete(delete_tracked_download))
        .route("/{id}", get(get_queue_item).delete(remove_queue_item))
        .route("/{id}/grab", get(grab_release))
        .route("/{id}/import", axum::routing::post(import_queue_item))
        .route("/{id}/import-preview", get(get_import_preview))
        .route("/{id}/match", axum::routing::put(update_match))
        .route("/{id}/files", get(get_queue_files))
}
