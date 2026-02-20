//! API v3 routes (legacy compatibility)

use crate::web::AppState;
use axum::{http::StatusCode, response::IntoResponse, Json, Router};
use std::sync::Arc;

mod autotagging;
mod backup;
mod blocklist;
mod calendar;
mod command;
mod config;
mod customfilter;
mod customformat;
mod delayprofile;
mod diskspace;
mod downloadclient;
mod episode;
mod episodefile;
mod filesystem;
mod health;
mod history;
mod importlist;
mod indexer;
mod indexerflag;
mod language;
mod languageprofile;
mod localization;
mod log;
mod manualimport;
mod mediacover;
mod metadata;
mod movie;
mod notification;
mod parse;
mod qualitydefinition;
mod qualityprofile;
mod queue;
mod release;
mod releaseprofile;
mod remotepathmapping;
mod rename;
mod rootfolder;
mod seasonpass;
mod series;
mod serieseditor;
mod serieslookup;
mod system;
mod tag;
mod update;
mod wanted;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Core resources
        .nest("/series", series::routes())
        .nest("/series/lookup", serieslookup::routes())
        .nest("/series/editor", serieseditor::routes())
        .nest("/episode", episode::routes())
        .nest("/episodeFile", episodefile::routes())
        .nest("/episodefile", episodefile::routes()) // lowercase alias
        .nest("/calendar", calendar::routes())
        .nest("/wanted", wanted::routes())
        .nest("/queue", queue::routes())
        .nest("/history", history::routes())
        .nest("/parse", parse::routes())
        .nest("/release", release::routes())
        .nest("/rename", rename::routes())
        .nest("/manualImport", manualimport::routes())
        .nest("/manualimport", manualimport::routes()) // lowercase alias
        .nest("/seasonPass", seasonpass::routes())
        .nest("/seasonpass", seasonpass::routes()) // lowercase alias
        // Configuration
        .nest("/config", config::routes())
        .nest("/qualityProfile", qualityprofile::routes())
        .nest("/qualityprofile", qualityprofile::routes()) // lowercase alias
        .nest("/qualityDefinition", qualitydefinition::routes())
        .nest("/qualitydefinition", qualitydefinition::routes()) // lowercase alias
        .nest("/delayProfile", delayprofile::routes())
        .nest("/delayprofile", delayprofile::routes()) // lowercase alias
        .nest("/releaseProfile", releaseprofile::routes())
        .nest("/releaseprofile", releaseprofile::routes()) // lowercase alias
        .nest("/languageProfile", languageprofile::routes())
        .nest("/languageprofile", languageprofile::routes()) // lowercase alias
        .nest("/language", language::routes())
        .nest("/customFormat", customformat::routes())
        .nest("/customformat", customformat::routes()) // lowercase alias
        .nest("/customFilter", customfilter::routes())
        .nest("/customfilter", customfilter::routes()) // lowercase alias
        .nest("/autoTagging", autotagging::routes())
        .nest("/autotagging", autotagging::routes()) // lowercase alias
        // Providers
        .nest("/downloadClient", downloadclient::routes())
        .nest("/downloadclient", downloadclient::routes()) // lowercase alias
        .nest("/indexer", indexer::routes())
        .nest("/indexerFlag", indexerflag::routes())
        .nest("/indexerflag", indexerflag::routes()) // lowercase alias
        .nest("/importList", importlist::routes())
        .nest("/importlist", importlist::routes()) // lowercase alias
        .nest("/notification", notification::routes())
        .nest("/metadata", metadata::routes())
        // Movies (Radarr v3 compat)
        .nest("/movie/lookup", movie::lookup_routes())
        .nest("/movie", movie::routes())
        // System
        .nest("/system", system::routes())
        .nest("/system/backup", backup::routes())
        .nest("/health", health::routes())
        .nest("/diskSpace", diskspace::routes())
        .nest("/diskspace", diskspace::routes()) // lowercase alias
        .nest("/update", update::routes())
        .nest("/log", log::routes())
        .nest("/command", command::routes())
        .nest("/fileSystem", filesystem::routes())
        .nest("/filesystem", filesystem::routes()) // lowercase alias
        .nest("/MediaCover", mediacover::routes())
        .nest("/mediaCover", mediacover::routes()) // camelCase alias
        .nest("/mediacover", mediacover::routes()) // lowercase alias
        // Other
        .nest("/localization", localization::routes())
        .nest("/tag", tag::routes())
        .nest("/rootFolder", rootfolder::routes())
        .nest("/rootfolder", rootfolder::routes()) // lowercase alias
        .nest("/remotePathMapping", remotepathmapping::routes())
        .nest("/remotepathmapping", remotepathmapping::routes()) // lowercase alias
        .nest("/blocklist", blocklist::routes())
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
