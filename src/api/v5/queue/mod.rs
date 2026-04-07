//! Queue API endpoints (v5)

mod common;
mod fetch;
mod files;
mod grab;
mod import;
mod list;
mod match_handler;
mod preview;
mod remove;
mod tracked;

#[allow(unused_imports)]
pub use common::*;
#[allow(unused_imports)]
pub use files::QueueFileResource;
#[allow(unused_imports)]
pub use match_handler::UpdateMatchRequest;
#[allow(unused_imports)]
pub use preview::{
    EpisodeOverride, ImportPreviewEpisode, ImportPreviewFile, ImportPreviewMovie,
    ImportPreviewResponse, ImportPreviewSeries, ImportQueueBody,
};
#[allow(unused_imports)]
pub use tracked::TrackedDeleteQuery;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list::list_queue).delete(remove::remove_from_queue))
        .route("/status", get(list::get_queue_status))
        .route("/details", get(list::get_queue_details))
        .route(
            "/tracked",
            axum::routing::delete(tracked::clear_tracked_downloads),
        )
        .route(
            "/tracked/{id}",
            axum::routing::delete(tracked::delete_tracked_download),
        )
        .route(
            "/{id}",
            get(list::get_queue_item).delete(remove::remove_queue_item),
        )
        .route("/{id}/grab", get(grab::grab_release))
        .route(
            "/{id}/import",
            axum::routing::post(import::import_queue_item),
        )
        .route("/{id}/import-preview", get(preview::get_import_preview))
        .route(
            "/{id}/match",
            axum::routing::put(match_handler::update_match),
        )
        .route("/{id}/files", get(files::get_queue_files))
}
