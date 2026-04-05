//! API v5 routes
//! Latest version of the pir9 API

use crate::web::AppState;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use std::sync::Arc;

pub mod blocklist;
pub mod calendar;
pub mod command;
pub mod config;
pub mod customfilter;
pub mod customformat;
pub mod diskspace;
pub mod download;
pub mod episodefile;
pub mod episodes;
pub mod filesystem;
pub mod health;
pub mod history;
pub mod imdb;
pub mod importexclusion;
pub mod importlist;
pub mod indexers;
pub mod localization;
pub mod log;
pub mod manualimport;
pub mod movies;
pub mod music;
pub mod musicbrainz;
pub mod notification;
pub mod parse;
pub mod podcast;
pub mod profile;
pub mod quality;
pub mod queue;
pub mod release;
pub mod remotepathmapping;
pub mod rootfolder;
pub mod seasonpass;
pub mod series;
pub mod settings;
pub mod system;
pub mod tag;
pub mod update;
pub mod wanted;

/// Create v5 API router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Core resources
        .nest("/series", series::routes())
        .nest("/movie", movies::routes())
        .nest("/artist", music::routes())
        .nest("/album", music::album_routes())
        .nest("/track", music::track_routes())
        .nest("/podcast", podcast::routes())
        .nest("/episode", episodes::routes())
        .nest("/episodeFile", episodefile::routes())
        .nest("/episodefile", episodefile::routes()) // lowercase alias
        .nest("/calendar", calendar::routes())
        .nest("/wanted", wanted::routes())
        .nest("/queue", queue::routes())
        .nest("/history", history::routes())
        .nest("/parse", parse::routes())
        .nest("/release", release::routes())
        .nest("/manualImport", manualimport::routes())
        .nest("/manualimport", manualimport::routes()) // lowercase alias
        .nest("/seasonPass", seasonpass::routes())
        .nest("/seasonpass", seasonpass::routes()) // lowercase alias
        // Configuration
        .nest("/config", config::routes())
        .nest("/settings", settings::routes())
        .nest("/qualityProfile", profile::quality_profile_routes())
        .nest("/qualityprofile", profile::quality_profile_routes()) // lowercase alias
        .nest("/qualityDefinition", quality::routes())
        .nest("/qualitydefinition", quality::routes()) // lowercase alias
        .nest("/delayProfile", profile::delay_profile_routes())
        .nest("/delayprofile", profile::delay_profile_routes()) // lowercase alias
        .nest("/releaseProfile", profile::release_profile_routes())
        .nest("/releaseprofile", profile::release_profile_routes()) // lowercase alias
        .nest("/customFormat", customformat::routes())
        .nest("/customformat", customformat::routes()) // lowercase alias
        .nest("/customFilter", customfilter::routes())
        .nest("/customfilter", customfilter::routes()) // lowercase alias
        // Providers
        .nest("/downloadClient", download::routes())
        .nest("/downloadclient", download::routes()) // lowercase alias
        .nest("/indexer", indexers::routes())
        .nest("/notification", notification::routes())
        // System
        .nest("/system", system::routes())
        .nest("/health", health::routes())
        .nest("/diskSpace", diskspace::routes())
        .nest("/diskspace", diskspace::routes()) // lowercase alias
        .nest("/update", update::routes())
        .nest("/log", log::routes())
        .nest("/command", command::routes())
        .nest("/fileSystem", filesystem::routes())
        .nest("/filesystem", filesystem::routes()) // lowercase alias
        // IMDB local database
        .nest("/imdb", imdb::routes())
        // MusicBrainz service
        .nest("/musicbrainz", musicbrainz::routes())
        // Other
        .nest("/localization", localization::routes())
        .nest("/tag", tag::routes())
        .nest("/rootFolder", rootfolder::routes())
        .nest("/rootfolder", rootfolder::routes()) // lowercase alias
        .nest("/remotePathMapping", remotepathmapping::routes())
        .nest("/remotepathmapping", remotepathmapping::routes()) // lowercase alias
        .nest("/blocklist", blocklist::routes())
        .nest("/importExclusion", importexclusion::routes())
        .nest("/importexclusion", importexclusion::routes()) // lowercase alias
        .nest("/importList", importlist::routes())
        .nest("/importlist", importlist::routes()) // lowercase alias
        // Rename (top-level for Sonarr compat, also available under /episodefile/rename)
        .route(
            "/rename",
            get(episodefile::get_rename_preview).put(episodefile::execute_rename),
        )
        // Fallback for unknown API endpoints - return JSON 404, not HTML
        .fallback(api_not_found)
}

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "message": "Resource not found"
        })),
    )
}
