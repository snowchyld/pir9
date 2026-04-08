//! File listing handler — returns files for a download from the client.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;

use super::fetch::fetch_all_downloads;
use crate::core::datastore::repositories::DownloadClientRepository;
use crate::core::download::clients::create_client_from_model;
use crate::core::queue::UNTRACKED_ID_BASE;
use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueFileResource {
    pub name: String,
    pub size: i64,
}

/// GET /api/v5/queue/{id}/files
/// Returns the file list for a download (from the download client)
pub(super) async fn get_queue_files(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<QueueFileResource>>, StatusCode> {
    let client_repo = DownloadClientRepository::new(state.db.clone());

    // Resolve download_id and client for this queue item
    let (download_id, client_id) = if id < UNTRACKED_ID_BASE {
        // Tracked download
        match state.tracked.find_by_id(id).await {
            Some(td) => (td.download_id, td.client_id),
            None => return Err(StatusCode::NOT_FOUND),
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
