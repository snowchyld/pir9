//! Tracked download management handlers — clear and delete operations.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;

use super::common::QueueActionResponse;
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
    // In-memory stores no longer track status values — completed/imported
    // records live in history. Clearing suppressed entries allows those
    // downloads to reappear in the queue for reimport.
    let removed = state.tracked.suppressed.remove_where(|_| true).await;

    tracing::info!(
        "Cleared {} suppressed download records (status filter: {:?})",
        removed.len(),
        query.status
    );
    Json(QueueActionResponse { success: true })
}

/// Delete a single tracked download record by ID.
pub(super) async fn delete_tracked_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<QueueActionResponse> {
    if state.tracked.remove_by_id(id).await {
        tracing::info!("Deleted tracked download record: {}", id);
        Json(QueueActionResponse { success: true })
    } else {
        tracing::warn!("Tracked download {} not found for deletion", id);
        Json(QueueActionResponse { success: false })
    }
}
