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
    pub episode_ordering: String,
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
    pub file_hash: Option<String>, // BLAKE3 content hash
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
    pub file_hash: Option<String>, // BLAKE3 content hash
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
    pub remove_completed_downloads: bool,
    pub remove_failed_downloads: bool,
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
    pub order: i32,
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
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub movie_id: Option<i64>,
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
    pub content_type: String,
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
    pub movie_id: Option<i64>, // FK to movies (for movie downloads)
    pub content_type: String,  // series, movie, anime, music, audiobook, podcast
}

/// Import exclusion database model
/// Tracks movies/series that should be excluded from automatic import
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportExclusionDbModel {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    pub title: String,
    pub year: Option<i32>,
    pub content_type: String,
    pub added: DateTime<Utc>,
}

/// Import list database model
/// Configuration for external lists (IMDB, Trakt) that auto-add content
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportListDbModel {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub list_type: String,
    pub list_url: Option<String>,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub search_on_add: bool,
    pub content_type: String,
    pub sync_interval_hours: i32,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub tags: String,
}

/// Release profile database model
/// Preferred words, required/ignored terms for release scoring
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReleaseProfileDbModel {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub required: String,  // JSON array of required terms
    pub ignored: String,   // JSON array of ignored terms
    pub preferred: String, // JSON array of {key: term, value: score}
    pub include_preferred_when_renaming: bool,
    pub indexer_id: i64,
    pub tags: String,
}

/// Artist database model (music domain)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArtistDbModel {
    pub id: i64,
    pub musicbrainz_id: Option<String>,
    pub name: String,
    pub clean_name: String,
    pub sort_name: String,
    pub overview: Option<String>,
    pub artist_type: String,
    pub status: String,
    pub genres: String,  // JSON serialized
    pub images: String,  // JSON serialized
    pub tags: String,    // JSON serialized
    pub path: String,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub title_slug: String,
}

/// Album database model (music domain)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlbumDbModel {
    pub id: i64,
    pub artist_id: i64,
    pub musicbrainz_id: Option<String>,
    pub title: String,
    pub clean_title: String,
    pub album_type: String,
    pub secondary_types: String,  // JSON serialized array of secondary types
    pub release_date: Option<NaiveDate>,
    pub genres: String,  // JSON serialized
    pub images: String,  // JSON serialized
    pub monitored: bool,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub title_slug: String,
}

/// Track database model (music domain)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrackDbModel {
    pub id: i64,
    pub album_id: i64,
    pub artist_id: i64,
    pub title: String,
    pub track_number: i32,
    pub disc_number: i32,
    pub duration_ms: Option<i32>,
    pub has_file: bool,
    pub track_file_id: Option<i64>,
    pub monitored: bool,
    pub air_date_utc: Option<DateTime<Utc>>,
}

/// Track file database model (music domain)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrackFileDbModel {
    pub id: i64,
    pub artist_id: i64,
    pub album_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub quality: String,            // JSON serialized
    pub media_info: Option<String>, // JSON serialized
    pub date_added: DateTime<Utc>,
}

/// Podcast database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PodcastDbModel {
    pub id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub author: Option<String>,
    pub feed_url: String,
    pub website_url: Option<String>,
    pub genres: String,  // JSON serialized
    pub images: String,  // JSON serialized
    pub tags: String,    // JSON serialized
    pub path: String,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub title_slug: String,
}

/// Podcast episode database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PodcastEpisodeDbModel {
    pub id: i64,
    pub podcast_id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub episode_number: Option<i32>,
    pub season_number: i32,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
    pub download_url: Option<String>,
    pub file_size: Option<i64>,
    pub has_file: bool,
    pub podcast_file_id: Option<i64>,
    pub monitored: bool,
    pub guid: Option<String>,
}

/// Podcast file database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PodcastFileDbModel {
    pub id: i64,
    pub podcast_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub quality: String,            // JSON serialized
    pub media_info: Option<String>, // JSON serialized
    pub date_added: DateTime<Utc>,
}

/// Audiobook database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AudiobookDbModel {
    pub id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub overview: Option<String>,
    pub publisher: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub duration_ms: Option<i64>,
    pub release_date: Option<NaiveDate>,
    pub genres: String,  // JSON serialized
    pub images: String,  // JSON serialized
    pub tags: String,    // JSON serialized
    pub path: String,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub has_file: bool,
    pub audiobook_file_id: Option<i64>,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub title_slug: String,
}

/// Audiobook chapter database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AudiobookChapterDbModel {
    pub id: i64,
    pub audiobook_id: i64,
    pub title: String,
    pub chapter_number: i32,
    pub duration_ms: Option<i64>,
    pub has_file: bool,
    pub monitored: bool,
}

/// Audiobook file database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AudiobookFileDbModel {
    pub id: i64,
    pub audiobook_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub quality: String,            // JSON serialized
    pub media_info: Option<String>, // JSON serialized
    pub date_added: DateTime<Utc>,
}
