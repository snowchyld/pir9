//! Queue item removal handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::Json,
};

use super::common::*;
use super::fetch::fetch_all_downloads;
use crate::core::datastore::repositories::{DownloadClientRepository, TrackedDownloadRepository};
use crate::core::download::clients::create_client_from_model;
use crate::core::queue::TrackedDownloadService;
use crate::web::AppState;

pub(super) async fn remove_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<RemoveFromQueueQuery>,
) -> Json<QueueActionResponse> {
    let service = TrackedDownloadService::new(state.db.clone(), state.tracked.clone());

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

    // Fallback for untracked downloads (id >= 10000)
    let downloads = fetch_all_downloads(&state, true).await;
    if let Some(download) = downloads.iter().find(|d| d.id == id) {
        if query.remove_from_client {
            // Remove from download client
            if let (Some(client_name), Some(download_id)) =
                (&download.download_client, &download.download_id)
            {
                let repo = DownloadClientRepository::new(state.db.clone());
                if let Ok(clients) = repo.get_all().await {
                    for db_client in clients.iter().filter(|c| c.name == *client_name) {
                        if let Ok(client) = create_client_from_model(db_client) {
                            let _ = client.remove(download_id, true).await;
                        }
                    }
                }
            }
        } else {
            // Create a tracked_downloads record with status=Ignored (7) so the
            // item moves to the Completed tab instead of reappearing in the queue
            let client_repo = DownloadClientRepository::new(state.db.clone());
            let mut client_id: i64 = 0;
            if let (Some(client_name), Ok(clients)) =
                (&download.download_client, client_repo.get_all().await)
            {
                if let Some(c) = clients.iter().find(|c| c.name == *client_name) {
                    client_id = c.id;
                }
            }

            let td_repo = TrackedDownloadRepository::new(state.db.clone());
            let model = crate::core::datastore::models::TrackedDownloadDbModel {
                id: 0,
                download_id: download.download_id.clone().unwrap_or_default(),
                download_client_id: client_id,
                series_id: download.series_id.unwrap_or(0),
                episode_ids: if let Some(ep_id) = download.episode_id {
                    serde_json::to_string(&vec![ep_id]).unwrap_or_else(|_| "[]".to_string())
                } else {
                    "[]".to_string()
                },
                title: download.title.clone(),
                indexer: download.indexer.clone(),
                size: download.size as i64,
                protocol: if download.protocol == "usenet" { 1 } else { 2 },
                quality: serde_json::to_string(&download.quality)
                    .unwrap_or_else(|_| "{}".to_string()),
                languages: serde_json::to_string(&download.languages)
                    .unwrap_or_else(|_| "[]".to_string()),
                status: 7, // Ignored
                status_messages: "[]".to_string(),
                error_message: None,
                output_path: download.output_path.clone(),
                is_upgrade: false,
                added: chrono::Utc::now(),
                movie_id: download.movie_id,
                artist_id: download.artist_id,
                audiobook_id: None,
                content_type: download.content_type.clone(),
            };

            if let Err(e) = td_repo.insert(&model).await {
                tracing::warn!(
                    "Failed to create tracked record for untracked download {}: {}",
                    id,
                    e
                );
            } else {
                tracing::info!(
                    "Created tracked record (Ignored) for untracked download: {} ({})",
                    download.title,
                    id
                );
            }
        }
    }

    Json(QueueActionResponse { success: true })
}

pub(super) async fn remove_from_queue(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RemoveFromQueueQuery>,
) -> Json<QueueActionResponse> {
    // Bulk remove all from queue
    let service = TrackedDownloadService::new(state.db.clone(), state.tracked.clone());
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
