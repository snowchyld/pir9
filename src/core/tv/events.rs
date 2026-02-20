#![allow(dead_code)]
//! TV domain events
//! Events related to series and episodes

use serde::{Deserialize, Serialize};

/// Series added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesAddedEvent {
    pub series_id: i64,
    pub title: String,
    pub tvdb_id: i64,
}

/// Series updated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesUpdatedEvent {
    pub series_id: i64,
    pub title: String,
    pub changes: Vec<String>,
}

/// Series deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesDeletedEvent {
    pub series_id: i64,
    pub title: String,
    pub deleted_files: bool,
}

/// Series refreshed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesRefreshedEvent {
    pub series_id: i64,
    pub title: String,
    pub episodes_added: i32,
    pub episodes_updated: i32,
}

/// Episode file imported event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeFileImportedEvent {
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub episode_file_id: i64,
    pub is_upgrade: bool,
    pub old_files: Vec<OldEpisodeFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldEpisodeFile {
    pub episode_file_id: i64,
    pub path: String,
    pub size: i64,
}

/// Episode file deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeFileDeletedEvent {
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub episode_file_id: i64,
    pub reason: DeleteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteReason {
    MissingFromDisk,
    Manual,
    Upgrade,
}

/// Episode monitored status changed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeMonitoredStatusChangedEvent {
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub monitored: bool,
}

/// Season monitored status changed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonMonitoredStatusChangedEvent {
    pub series_id: i64,
    pub season_number: i32,
    pub monitored: bool,
}
