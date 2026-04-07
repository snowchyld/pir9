#![allow(dead_code)]
//! Music domain events
//! Events related to artists, albums, and tracks

use serde::{Deserialize, Serialize};

/// Artist added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistAddedEvent {
    pub artist_id: i64,
    pub name: String,
    pub musicbrainz_id: Option<String>,
}

/// Artist updated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistUpdatedEvent {
    pub artist_id: i64,
    pub name: String,
    pub changes: Vec<String>,
}

/// Artist deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistDeletedEvent {
    pub artist_id: i64,
    pub name: String,
    pub deleted_files: bool,
}

/// Artist refreshed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistRefreshedEvent {
    pub artist_id: i64,
    pub name: String,
}

/// Track file imported event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackFileImportedEvent {
    pub artist_id: i64,
    pub album_id: i64,
    pub track_file_id: i64,
    pub is_upgrade: bool,
}

/// Track file deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackFileDeletedEvent {
    pub artist_id: i64,
    pub track_file_id: i64,
    pub reason: DeleteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteReason {
    MissingFromDisk,
    Manual,
    Upgrade,
}
