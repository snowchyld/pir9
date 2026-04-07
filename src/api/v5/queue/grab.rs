//! Re-grab handler — re-downloads a release for a tracked download.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Json,
};

use super::common::QueueActionResponse;
use crate::core::datastore::repositories::{EpisodeRepository, SeriesRepository};
use crate::core::queue::TrackedDownloadService;
use crate::web::AppState;

pub(super) async fn grab_release(
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
    let service = TrackedDownloadService::new(state.db.clone(), state.tracked.clone());
    let _ = service.remove(id, false, false).await;

    // Grab the best release (first in quality-sorted list)
    let best = &releases[0];
    match service
        .grab_release(best, episode_ids, tracked.movie_id, &tracked.content_type)
        .await
    {
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
