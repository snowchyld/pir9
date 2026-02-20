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
    DownloadClientRepository, EpisodeRepository, SeriesRepository,
};
use crate::core::download::clients::{create_client_from_model, DownloadState};
use crate::core::parser::{normalize_title, parse_title, title_matches_series};
use crate::core::queue::{
    Protocol as QueueProtocol, QueueStatus, TrackedDownloadService, TrackedDownloadState,
    TrackedDownloadStatus,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<QueueEpisodeResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<QueueSeriesResource>,
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
        episode: None,
        series: None,
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

    // Get tracked downloads with live status merged
    match service.get_queue().await {
        Ok(queue_items) => {
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

    // Collect download IDs that are already tracked
    let tracked_ids: HashSet<String> = all_downloads
        .iter()
        .filter_map(|d| d.download_id.clone())
        .collect();

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
        let mut id_counter = (all_downloads.len() as i64) + 10000;

        for db_client in clients.iter().filter(|c| c.enable) {
            // Parse the configured category from client settings
            let client_category: Option<String> =
                serde_json::from_str::<serde_json::Value>(&db_client.settings)
                    .ok()
                    .and_then(|s| {
                        s.get("tvCategory")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .filter(|s| !s.is_empty());

            match create_client_from_model(db_client) {
                Ok(client) => match client.get_downloads().await {
                    Ok(downloads) => {
                        let protocol = if db_client.protocol == 1 {
                            "usenet"
                        } else {
                            "torrent"
                        };

                        for dl in downloads {
                            if tracked_ids.contains(&dl.id) {
                                continue;
                            }

                            // Skip downloads that don't match the configured category
                            if let Some(ref expected_cat) = client_category {
                                let dl_cat = dl.category.as_deref().unwrap_or("");
                                if !dl_cat.eq_ignore_ascii_case(expected_cat) {
                                    continue;
                                }
                            }

                            let status = match dl.status {
                                DownloadState::Queued => "queued",
                                DownloadState::Paused => "paused",
                                DownloadState::Downloading => "downloading",
                                DownloadState::Seeding => "seeding",
                                DownloadState::Completed => "completed",
                                DownloadState::Failed => "failed",
                                DownloadState::Warning => "warning",
                            };

                            let tracked_state = match dl.status {
                                DownloadState::Queued => "importPending",
                                DownloadState::Downloading => "downloading",
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

                                for series in &all_series {
                                    if title_matches_series(info, &series.title)
                                        || title_matches_series(info, &series.clean_title)
                                    {
                                        matched_series_id = Some(series.id);

                                        // Standard S01E02 matching
                                        if let Some(season) = info.season_number {
                                            if !info.episode_numbers.is_empty() {
                                                let ep_num = info.episode_numbers[0];
                                                if let Ok(Some(ep)) = episode_repo
                                                    .get_by_series_season_episode(
                                                        series.id, season, ep_num,
                                                    )
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

                                        break;
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

                            // Check if matched episode(s) already have files in the library
                            let mut episode_has_file_val = false;
                            if let Some(ep_id) = matched_episode_id {
                                if let Ok(Some(ep)) = episode_repo.get_by_id(ep_id).await {
                                    episode_has_file_val = ep.has_file;
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
                                                episode_has_file_val = true;
                                            }
                                        }
                                    }
                                }
                            }

                            // Skip completed/seeding downloads where episode(s) already imported
                            if episode_has_file_val
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
                                        messages: vec![dl
                                            .error_message
                                            .clone()
                                            .unwrap_or_default()],
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
                                episode_has_file: episode_has_file_val,
                                episode: parsed_episode,
                                series: parsed_series,
                            });

                            id_counter += 1;
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Failed to get downloads from {}: {}", db_client.name, e);
                    }
                },
                Err(e) => {
                    tracing::debug!("Failed to create client for {}: {}", db_client.name, e);
                }
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

    // Enrich with episode/series metadata
    if include_episode || include_series {
        let episode_repo = EpisodeRepository::new(state.db.clone());
        let series_repo = SeriesRepository::new(state.db.clone());

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
                        dl.series = Some(QueueSeriesResource {
                            id: s.id,
                            title_slug: s.title_slug.clone(),
                            title: s.title,
                        });
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
    match service.grab_release(best, episode_ids).await {
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
) -> Json<QueueActionResponse> {
    use crate::core::datastore::repositories::TrackedDownloadRepository;
    use crate::core::download::import::ImportService;

    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let client_repo = DownloadClientRepository::new(state.db.clone());
    let import_service = ImportService::new(state.db.clone(), state.config.read().media.clone());

    // Find the download — either tracked (id < 10000) or untracked
    let (download_id, download_client_id, title) = if id < 10000 {
        // Tracked download — look up from DB
        match td_repo.get_by_id(id).await {
            Ok(Some(td)) => (td.download_id, td.download_client_id, td.title),
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
                (dl_id, client_id, dl.title)
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
        if let Ok(mappings) = mapping_repo.get_all().await {
            for m in &mappings {
                if mapped.starts_with(&m.remote_path) {
                    mapped = mapped.replacen(&m.remote_path, &m.local_path, 1);
                    tracing::debug!("Import: mapped path '{}' -> '{}'", raw_output_path, mapped);
                    break;
                }
            }
        }
        mapped
    };

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

    let pending = crate::core::download::import::PendingImport {
        download_id: download_id.clone(),
        download_client_id,
        download_client_name: client_model.name.clone(),
        title: title.clone(),
        output_path: std::path::PathBuf::from(&output_path),
        parsed_info: parsed,
        series,
        episodes,
    };

    // Run the import
    match import_service.import(&pending).await {
        Ok(result) if result.success => {
            tracing::info!(
                "Imported '{}': {} files, {} episodes",
                title,
                result.files_imported,
                result.episode_ids.len()
            );

            // Update tracked download state to Imported (if tracked)
            if id < 10000 {
                let _ = td_repo
                    .update_status(
                        id,
                        crate::core::queue::TrackedDownloadState::Imported as i32,
                        "[]",
                        None,
                    )
                    .await;
            }

            Json(QueueActionResponse { success: true })
        }
        Ok(result) => {
            let msg = result
                .error_message
                .unwrap_or_else(|| "Unknown error".to_string());
            tracing::warn!("Import failed for '{}': {}", title, msg);
            Json(QueueActionResponse { success: false })
        }
        Err(e) => {
            tracing::warn!("Import error for '{}': {}", title, e);
            Json(QueueActionResponse { success: false })
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_queue).delete(remove_from_queue))
        .route("/status", get(get_queue_status))
        .route("/details", get(get_queue_details))
        .route("/{id}", get(get_queue_item).delete(remove_queue_item))
        .route("/{id}/grab", get(grab_release))
        .route("/{id}/import", axum::routing::post(import_queue_item))
}
