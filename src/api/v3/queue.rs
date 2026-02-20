//! Queue API endpoints
//! Shows download status from all configured download clients

use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeRepository, SeriesRepository, TrackedDownloadRepository,
};
use crate::core::download::clients::{create_client_from_model, DownloadState};
use crate::core::parser::{parse_title, title_matches_series};
use crate::core::queue::{
    Protocol as QueueProtocol, QueueStatus, TrackedDownloadService, TrackedDownloadState,
    TrackedDownloadStatus,
};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct QueueQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_unknown_series_items: Option<bool>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueResource {
    pub id: i32,
    pub series_id: Option<i32>,
    pub episode_id: Option<i32>,
    pub series: Option<serde_json::Value>,
    pub episode: Option<serde_json::Value>,
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
pub struct QueueResourcePagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<QueueResource>,
}

/// Convert QueueItem from service to QueueResource for API
fn queue_item_to_resource(
    item: &crate::core::queue::QueueItem,
    _series_repo: &SeriesRepository,
    _episode_repo: &EpisodeRepository,
) -> QueueResource {
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
        id: item.id as i32,
        series_id: if item.series_id > 0 {
            Some(item.series_id as i32)
        } else {
            None
        },
        episode_id: if item.episode_id > 0 {
            Some(item.episode_id as i32)
        } else {
            None
        },
        series: None,  // TODO: Include full series data if requested
        episode: None, // TODO: Include full episode data if requested
        languages: vec![LanguageResource {
            id: 1,
            name: "English".to_string(),
        }], // TODO: Parse from item
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
    }
}

/// Fetch downloads from all enabled download clients
/// Combines tracked downloads (from database) with untracked downloads (direct from clients)
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
                all_downloads.push(queue_item_to_resource(&item, &series_repo, &episode_repo));
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
        let mut id_counter = (all_downloads.len() as i32) + 10000; // Offset to avoid ID collision

        for db_client in clients.iter().filter(|c| c.enable) {
            match create_client_from_model(db_client) {
                Ok(client) => {
                    match client.get_downloads().await {
                        Ok(downloads) => {
                            let protocol = if db_client.protocol == 1 {
                                "usenet"
                            } else {
                                "torrent"
                            };

                            for dl in downloads {
                                // Skip if already tracked
                                if tracked_ids.contains(&dl.id) {
                                    continue;
                                }

                                let status = match dl.status {
                                    DownloadState::Queued => "queued",
                                    DownloadState::Paused => "paused",
                                    DownloadState::Downloading => "downloading",
                                    DownloadState::Stalled => "warning",
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

                                // Parse the download title to match series/episode
                                let parsed = parse_title(&dl.name);
                                let mut matched_series_id: Option<i32> = None;
                                let mut matched_episode_id: Option<i32> = None;
                                let mut episode_has_file = false;
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
                                            matched_series_id = Some(series.id as i32);

                                            if let Some(season) = info.season_number {
                                                if !info.episode_numbers.is_empty() {
                                                    let ep_num = info.episode_numbers[0];
                                                    if let Ok(Some(ep)) = episode_repo
                                                        .get_by_series_season_episode(
                                                            series.id, season, ep_num,
                                                        )
                                                        .await
                                                    {
                                                        matched_episode_id = Some(ep.id as i32);
                                                        episode_has_file = ep.has_file;
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }

                                all_downloads.push(QueueResource {
                                    id: id_counter,
                                    series_id: matched_series_id,
                                    episode_id: matched_episode_id,
                                    series: None,
                                    episode: None,
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
                                    episode_has_file,
                                });

                                id_counter += 1;
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Failed to get downloads from {}: {}",
                                db_client.name,
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to create client for {}: {}", db_client.name, e);
                }
            }
        }
    }

    // Filter out seeding/completed downloads where episode already has file (imported).
    // These are "done" — the file is on disk, the torrent is just seeding.
    let before = all_downloads.len();
    all_downloads.retain(|d| {
        if !d.episode_has_file {
            return true;
        }
        // Hide if status is seeding/completed (untracked) or state is importPending (tracked)
        let dominated = matches!(d.status.as_str(), "seeding" | "completed")
            || d.tracked_download_state.as_deref() == Some("importPending");
        !dominated
    });
    let filtered = before - all_downloads.len();
    if filtered > 0 {
        tracing::debug!("Queue: filtered {} imported+seeding downloads", filtered);
    }

    all_downloads
}

/// GET /api/v3/queue
pub async fn get_queue(
    State(state): State<Arc<AppState>>,
    query: Query<QueueQuery>,
) -> Json<QueueResourcePagingResource> {
    let include_unknown = query.include_unknown_series_items.unwrap_or(true);
    let all_downloads = fetch_all_downloads(&state, include_unknown).await;

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

    let total_records = all_downloads.len() as i32;

    // Apply pagination
    let start = ((page - 1) * page_size) as usize;
    let end = (start + page_size as usize).min(all_downloads.len());

    let records = if start < all_downloads.len() {
        all_downloads[start..end].to_vec()
    } else {
        vec![]
    };

    Json(QueueResourcePagingResource {
        page,
        page_size,
        sort_key: query.sort_key.clone().unwrap_or("timeleft".to_string()),
        sort_direction: query
            .sort_direction
            .clone()
            .unwrap_or("ascending".to_string()),
        total_records,
        records,
    })
}

/// GET /api/v3/queue/details
pub async fn get_queue_details(State(state): State<Arc<AppState>>) -> Json<Vec<QueueResource>> {
    let downloads = fetch_all_downloads(&state, true).await;
    Json(downloads)
}

/// GET /api/v3/queue/status
pub async fn get_queue_status(State(state): State<Arc<AppState>>) -> Json<QueueStatusResource> {
    let downloads = fetch_all_downloads(&state, true).await;

    let total_count = downloads.len() as i32;
    let errors = downloads.iter().any(|d| d.status == "failed");
    let warnings = downloads.iter().any(|d| d.status == "warning");

    Json(QueueStatusResource {
        total_count,
        count: total_count,
        unknown_count: total_count, // All are "unknown" until we match to series
        errors,
        warnings,
        unknown_errors: errors,
        unknown_warnings: warnings,
    })
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct DeleteQueueQuery {
    #[serde(default)]
    pub remove_from_client: bool,
    #[serde(default)]
    pub blocklist: bool,
    #[serde(default)]
    pub skip_redownload: bool,
    #[serde(default)]
    pub change_category: bool,
}

/// DELETE /api/v3/queue/:id
/// Remove item from download client
pub async fn delete_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Query(query): Query<DeleteQueueQuery>,
) -> Json<serde_json::Value> {
    let service = TrackedDownloadService::new(state.db.clone());
    let _tracked_repo = TrackedDownloadRepository::new(state.db.clone());

    // Check if this is a tracked download (ID < 10000)
    if id < 10000 {
        // Try to remove via service (handles both DB and client removal)
        if let Err(e) = service
            .remove(id as i64, query.remove_from_client, query.blocklist)
            .await
        {
            tracing::warn!("Failed to remove tracked download {}: {}", id, e);
        } else {
            tracing::info!("Removed tracked download {}", id);
            return Json(serde_json::json!({}));
        }
    }

    // Fall back to legacy behavior for untracked downloads
    let downloads = fetch_all_downloads(&state, true).await;

    if let Some(download) = downloads.iter().find(|d| d.id == id) {
        if let (Some(client_name), Some(download_id)) =
            (&download.download_client, &download.download_id)
        {
            let repo = DownloadClientRepository::new(state.db.clone());

            if let Ok(clients) = repo.get_all().await {
                for db_client in clients.iter().filter(|c| c.name == *client_name) {
                    if let Ok(client) = create_client_from_model(db_client) {
                        if let Err(e) = client.remove(download_id, query.remove_from_client).await {
                            tracing::warn!("Failed to remove download {}: {}", download_id, e);
                        } else {
                            tracing::info!("Removed download {} from {}", download_id, client_name);
                        }
                    }
                }
            }
        }
    }

    Json(serde_json::json!({}))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BulkDeleteRequest {
    pub ids: Vec<i32>,
    #[serde(default)]
    pub remove_from_client: bool,
    #[serde(default)]
    pub blocklist: bool,
    #[serde(default)]
    pub skip_redownload: bool,
    #[serde(default)]
    pub change_category: bool,
}

/// DELETE /api/v3/queue/bulk
pub async fn delete_queue_bulk(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeleteRequest>,
) -> Json<serde_json::Value> {
    let service = TrackedDownloadService::new(state.db.clone());

    for id in &body.ids {
        // Check if this is a tracked download (ID < 10000)
        if *id < 10000 {
            if let Err(e) = service
                .remove(*id as i64, body.remove_from_client, body.blocklist)
                .await
            {
                tracing::warn!("Failed to remove tracked download {}: {}", id, e);
            }
            continue;
        }

        // Fall back to legacy behavior for untracked downloads
        let downloads = fetch_all_downloads(&state, true).await;

        if let Some(download) = downloads.iter().find(|d| d.id == *id) {
            if let (Some(client_name), Some(download_id)) =
                (&download.download_client, &download.download_id)
            {
                let repo = DownloadClientRepository::new(state.db.clone());

                if let Ok(clients) = repo.get_all().await {
                    for db_client in clients.iter().filter(|c| c.name == *client_name) {
                        if let Ok(client) = create_client_from_model(db_client) {
                            let _ = client.remove(download_id, body.remove_from_client).await;
                        }
                    }
                }
            }
        }
    }

    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_queue))
        .route("/details", get(get_queue_details))
        .route("/status", get(get_queue_status))
        .route("/{id}", delete(delete_queue_item))
        .route("/bulk", delete(delete_queue_bulk))
}
