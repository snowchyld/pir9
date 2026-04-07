#![allow(dead_code)]
//! Podcast domain events
//! Events related to podcasts and podcast episodes

use serde::{Deserialize, Serialize};

/// Podcast added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastAddedEvent {
    pub podcast_id: i64,
    pub title: String,
    pub feed_url: String,
}

/// Podcast updated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastUpdatedEvent {
    pub podcast_id: i64,
    pub title: String,
    pub changes: Vec<String>,
}

/// Podcast deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastDeletedEvent {
    pub podcast_id: i64,
    pub title: String,
    pub deleted_files: bool,
}

/// Podcast refreshed event (feed re-fetched)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastRefreshedEvent {
    pub podcast_id: i64,
    pub title: String,
    pub episodes_added: i32,
    pub episodes_updated: i32,
}

/// Podcast episode added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastEpisodeAddedEvent {
    pub podcast_id: i64,
    pub episode_id: i64,
    pub title: String,
}

/// Podcast episode file imported event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastFileImportedEvent {
    pub podcast_id: i64,
    pub episode_id: i64,
    pub podcast_file_id: i64,
}

/// Podcast episode file deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastFileDeletedEvent {
    pub podcast_id: i64,
    pub podcast_file_id: i64,
    pub reason: DeleteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteReason {
    MissingFromDisk,
    Manual,
    Upgrade,
}
