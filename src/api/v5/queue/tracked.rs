//! Tracked download management handlers — clear and delete operations.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;

use super::common::QueueActionResponse;
use crate::core::datastore::repositories::TrackedDownloadRepository;
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedDeleteQuery {
    /// Filter by tracked download status (e.g. 4 = Imported)
    pub status: Option<i32>,
}

/// Clear tracked download records, optionally filtered by status.
/// This allows previously imported torrents to reappear in the queue for reimport.
pub(super) async fn clear_tracked_downloads(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrackedDeleteQuery>,
) -> Json<QueueActionResponse> {
    let td_repo = TrackedDownloadRepository::new(state.db.clone());

    let deleted = if let Some(status) = query.status {
        td_repo.delete_all_by_status(status).await.unwrap_or(0)
    } else {
        // Without a status filter, clear both Imported (4) and Ignored (7)
        let a = td_repo.delete_all_by_status(4).await.unwrap_or(0);
        let b = td_repo.delete_all_by_status(7).await.unwrap_or(0);
        a + b
    };

    tracing::info!(
        "Cleared {} tracked download records (status filter: {:?})",
        deleted,
        query.status
    );
    Json(QueueActionResponse { success: true })
}

/// Delete a single tracked download record by ID.
pub(super) async fn delete_tracked_download(
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
