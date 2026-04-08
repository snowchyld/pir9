//! Queue item removal handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::Json,
};

use super::common::*;
use super::fetch::fetch_all_downloads;
use crate::core::datastore::repositories::DownloadClientRepository;
use crate::core::download::clients::create_client_from_model;
use crate::core::queue::tracked::SuppressedRef;
use crate::core::queue::{TrackedDownloadService, UNTRACKED_ID_BASE};
use crate::web::AppState;

pub(super) async fn remove_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<RemoveFromQueueQuery>,
) -> Json<QueueActionResponse> {
    let service = TrackedDownloadService::new(state.db.clone(), state.tracked.clone());

    if id < UNTRACKED_ID_BASE {
        if let Err(e) = service
            .remove(id, query.remove_from_client, query.blocklist)
            .await
        {
            tracing::warn!("Failed to remove tracked download {}: {}", id, e);
        } else {
            return Json(QueueActionResponse { success: true });
        }
    }

    // Fallback for untracked downloads (id >= UNTRACKED_ID_BASE)
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
            // Suppress the download so it doesn't reappear in the queue.
            // Resolve the download client ID from its name.
            let client_repo = DownloadClientRepository::new(state.db.clone());
            let mut client_id: i64 = 0;
            if let (Some(client_name), Ok(clients)) =
                (&download.download_client, client_repo.get_all().await)
            {
                if let Some(c) = clients.iter().find(|c| c.name == *client_name) {
                    client_id = c.id;
                }
            }

            use crate::core::queue::tracked::TrackedDownload;
            let suppressed = TrackedDownload {
                id: 0,
                download_id: download.download_id.clone().unwrap_or_default(),
                client_id,
                content: SuppressedRef,
                title: download.title.clone(),
                quality: String::new(),
                indexer: download.indexer.clone(),
                added: chrono::Utc::now(),
                is_upgrade: false,
            };

            match state.tracked.suppressed.insert(suppressed).await {
                Ok(_) => {
                    tracing::info!(
                        "Suppressed untracked download: {} ({})",
                        download.title,
                        id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to suppress untracked download {}: {}",
                        id,
                        e
                    );
                }
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
        if download.id < UNTRACKED_ID_BASE {
            let _ = service
                .remove(download.id, query.remove_from_client, query.blocklist)
                .await;
        }
    }

    Json(QueueActionResponse { success: true })
}
