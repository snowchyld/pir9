#![allow(dead_code)]
//! Movie domain events
//! Events related to movies and movie files

use serde::{Deserialize, Serialize};

/// Movie added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieAddedEvent {
    pub movie_id: i64,
    pub title: String,
    pub tmdb_id: i64,
}

/// Movie updated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieUpdatedEvent {
    pub movie_id: i64,
    pub title: String,
    pub changes: Vec<String>,
}

/// Movie deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieDeletedEvent {
    pub movie_id: i64,
    pub title: String,
    pub deleted_files: bool,
}

/// Movie refreshed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieRefreshedEvent {
    pub movie_id: i64,
    pub title: String,
}

/// Movie file imported event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieFileImportedEvent {
    pub movie_id: i64,
    pub movie_file_id: i64,
    pub is_upgrade: bool,
}

/// Movie file deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieFileDeletedEvent {
    pub movie_id: i64,
    pub movie_file_id: i64,
    pub reason: DeleteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteReason {
    MissingFromDisk,
    Manual,
    Upgrade,
}
