#![allow(dead_code)]
//! Database models for SQLx
//! Entity definitions for database tables

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Series database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SeriesDbModel {
    pub id: i64,
    pub tvdb_id: i64,
    pub tv_rage_id: i64,
    pub tv_maze_id: i64,
    pub imdb_id: Option<String>,
    pub tmdb_id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub status: i32,
    pub overview: Option<String>,
    pub monitored: bool,
    pub monitor_new_items: i32,
    pub quality_profile_id: i64,
    pub language_profile_id: Option<i64>,
    pub season_folder: bool,
    pub series_type: i32,
    pub title_slug: String,
    pub path: String,
    pub root_folder_path: String,
    pub year: i32,
    pub first_aired: Option<NaiveDate>,
    pub last_aired: Option<NaiveDate>,
    pub runtime: i32,
    pub network: Option<String>,
    pub certification: Option<String>,
    pub use_scene_numbering: bool,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<i32>,
}

/// Episode database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EpisodeDbModel {
    pub id: i64,
    pub series_id: i64,
    pub tvdb_id: i64,
    pub episode_file_id: Option<i64>,
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_episode_number: Option<i32>,
    pub scene_absolute_episode_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub scene_season_number: Option<i32>,
    pub title: String,
    pub overview: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub runtime: i32,
    pub has_file: bool,
    pub monitored: bool,
    pub unverified_scene_numbering: bool,
    pub added: DateTime<Utc>,
    pub last_search_time: Option<DateTime<Utc>>,
    pub imdb_id: Option<String>,
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<i32>,
}

/// Episode file database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EpisodeFileDbModel {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub quality: String,            // JSON serialized
    pub languages: String,          // JSON serialized
    pub media_info: Option<String>, // JSON serialized
    pub original_file_path: Option<String>,
}

/// Movie database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MovieDbModel {
    pub id: i64,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub status: i32,
    pub overview: Option<String>,
    pub monitored: bool,
    pub quality_profile_id: i64,
    pub title_slug: String,
    pub path: String,
    pub root_folder_path: String,
    pub year: i32,
    pub release_date: Option<NaiveDate>,
    pub physical_release_date: Option<NaiveDate>,
    pub digital_release_date: Option<NaiveDate>,
    pub runtime: i32,
    pub studio: Option<String>,
    pub certification: Option<String>,
    pub genres: String, // JSON serialized
    pub tags: String,   // JSON serialized
    pub images: String, // JSON serialized
    pub has_file: bool,
    pub movie_file_id: Option<i64>,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<i32>,
}

/// Movie file database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MovieFileDbModel {
    pub id: i64,
    pub movie_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub quality: String,            // JSON serialized
    pub languages: String,          // JSON serialized
    pub media_info: Option<String>, // JSON serialized
    pub original_file_path: Option<String>,
    pub edition: Option<String>,
}

/// Episode to file mapping (for multi-episode files)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EpisodeFileMapping {
    pub episode_file_id: i64,
    pub episode_id: i64,
}

/// Season database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SeasonDbModel {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub monitored: bool,
}

/// Indexer database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IndexerDbModel {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub enable_rss: bool,
    pub enable_automatic_search: bool,
    pub enable_interactive_search: bool,
    pub protocol: i32,
    pub priority: i32,
    pub download_client_id: i64,
    pub settings: String, // JSON serialized
    pub tags: String,     // JSON serialized
}

/// Download client database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DownloadClientDbModel {
    pub id: i64,
    pub enable: bool,
    pub protocol: i32,
    pub priority: i32,
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub settings: String, // JSON serialized
    pub tags: String,     // JSON serialized
}

/// Notification database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationDbModel {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub on_grab: bool,
    pub on_download: bool,
    pub on_upgrade: bool,
    pub on_rename: bool,
    pub on_series_delete: bool,
    pub on_episode_file_delete: bool,
    pub on_episode_file_delete_for_upgrade: bool,
    pub on_health_issue: bool,
    pub on_health_restored: bool,
    pub on_manual_interaction_required: bool,
    pub on_application_update: bool,
    pub include_health_warnings: bool,
    pub settings: String, // JSON serialized
    pub tags: String,     // JSON serialized
}

/// Quality profile database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QualityProfileDbModel {
    pub id: i64,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: i32,
    pub items: String, // JSON serialized
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub format_items: String, // JSON serialized
}

/// Language profile database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LanguageProfileDbModel {
    pub id: i64,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: i32,
    pub languages: String, // JSON serialized
}

/// Delay profile database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DelayProfileDbModel {
    pub id: i64,
    pub enable_usenet: bool,
    pub enable_torrent: bool,
    pub preferred_protocol: i32,
    pub usenet_delay: i32,
    pub torrent_delay: i32,
    pub bypass_if_highest_quality: bool,
    pub bypass_if_above_custom_format_score: i32,
    pub tags: String, // JSON serialized
}

/// Custom format database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomFormatDbModel {
    pub id: i64,
    pub name: String,
    pub include_custom_format_when_renaming: bool,
    pub specifications: String, // JSON serialized
}

/// Tag database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TagDbModel {
    pub id: i64,
    pub label: String,
}

/// History database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HistoryDbModel {
    pub id: i64,
    pub series_id: i64,
    pub episode_id: i64,
    pub source_title: String,
    pub quality: String,        // JSON serialized
    pub languages: String,      // JSON serialized
    pub custom_formats: String, // JSON serialized
    pub custom_format_score: i32,
    pub quality_cutoff_not_met: bool,
    pub date: DateTime<Utc>,
    pub download_id: Option<String>,
    pub event_type: i32,
    pub data: String, // JSON serialized
}

/// Blocklist database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlocklistDbModel {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: String, // JSON serialized
    pub source_title: String,
    pub quality: String,        // JSON serialized
    pub languages: String,      // JSON serialized
    pub custom_formats: String, // JSON serialized
    pub custom_format_score: i32,
    pub protocol: i32,
    pub indexer: String,
    pub message: Option<String>,
    pub date: DateTime<Utc>,
}

/// Custom filter database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomFilterDbModel {
    pub id: i64,
    pub filter_type: String,
    pub label: String,
    pub filters: String, // JSON serialized
}

/// Remote path mapping database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RemotePathMappingDbModel {
    pub id: i64,
    pub host: String,
    pub remote_path: String,
    pub local_path: String,
}

/// Root folder database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RootFolderDbModel {
    pub id: i64,
    pub path: String,
    pub accessible: bool,
    pub free_space: Option<i64>,
    pub total_space: Option<i64>,
    pub unmapped_folders: Option<String>, // JSON serialized
}

/// Config database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConfigDbModel {
    pub key: String,
    pub value: String,
}

/// Log database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogDbModel {
    pub id: i64,
    pub time: DateTime<Utc>,
    pub level: String,
    pub logger: String,
    pub message: String,
    pub exception: Option<String>,
    pub exception_type: Option<String>,
}

/// Log file database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogFileDbModel {
    pub id: i64,
    pub filename: String,
    pub last_write_time: DateTime<Utc>,
}

/// Tracked download database model
/// Tracks downloads sent to clients with their series/episode relationships
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrackedDownloadDbModel {
    pub id: i64,
    pub download_id: String,           // ID from download client
    pub download_client_id: i64,       // FK to download_clients
    pub series_id: i64,                // FK to series
    pub episode_ids: String,           // JSON array of episode IDs
    pub title: String,                 // Release title
    pub indexer: Option<String>,       // Indexer name
    pub size: i64,                     // Size in bytes
    pub protocol: i32,                 // 1=Usenet, 2=Torrent
    pub quality: String,               // JSON QualityModel
    pub languages: String,             // JSON array of languages
    pub status: i32,                   // TrackedDownloadState enum
    pub status_messages: String,       // JSON array of StatusMessage
    pub error_message: Option<String>, // Error message if failed
    pub output_path: Option<String>,   // Download output path
    pub is_upgrade: bool,              // Whether this is an upgrade
    pub added: DateTime<Utc>,
}
