#![allow(dead_code)]
//! Database models for SQLx
//! Entity definitions for database tables

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// A String that SQLx can decode from JSONB columns.
///
/// PostgreSQL JSONB columns can't be directly decoded into `String` by SQLx
/// (String only supports TEXT/VARCHAR/etc). This newtype transparently converts
/// JSONB → String on read and String → JSONB on write, so all existing code
/// that uses `serde_json::from_str(&field)` / `serde_json::to_string()` works
/// unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonString(pub String);

impl JsonString {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

// Deref to str so &JsonString → &str — from_str(&field) works unchanged.
// We deref to str (not String) so .clone() on JsonString returns JsonString, not String.
impl std::ops::Deref for JsonString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<String> for JsonString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<JsonString> for String {
    fn from(js: JsonString) -> String {
        js.0
    }
}

impl AsRef<str> for JsonString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for JsonString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl std::fmt::Display for JsonString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// SQLx: Tell SQLx this type maps to JSONB
impl sqlx::Type<sqlx::Postgres> for JsonString {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("jsonb")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        *ty == sqlx::postgres::PgTypeInfo::with_name("jsonb")
            || *ty == sqlx::postgres::PgTypeInfo::with_name("json")
            || <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

// SQLx: Decode JSONB as String (Postgres sends JSONB as text over the wire with a version byte prefix)
impl sqlx::Decode<'_, sqlx::Postgres> for JsonString {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        // Try decoding as serde_json::Value first (JSONB binary format), then stringify
        let val = <serde_json::Value as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(JsonString(val.to_string()))
    }
}

// SQLx: Encode String as JSONB
impl sqlx::Encode<'_, sqlx::Postgres> for JsonString {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        // Parse the string as JSON and encode as JSONB
        let val: serde_json::Value = serde_json::from_str(&self.0)
            .unwrap_or(serde_json::Value::String(self.0.clone()));
        <serde_json::Value as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&val, buf)
    }
}

impl sqlx::postgres::PgHasArrayType for JsonString {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_jsonb")
    }
}

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
    pub quality: JsonString, // JSONB
    pub languages: JsonString, // JSONB
    pub media_info: Option<JsonString>, // JSONB
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
    pub genres: JsonString, // JSONB
    pub tags: JsonString, // JSONB
    pub images: JsonString, // JSONB
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
    pub quality: JsonString, // JSONB
    pub languages: JsonString, // JSONB
    pub media_info: Option<JsonString>, // JSONB
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
    pub settings: JsonString, // JSONB
    pub tags: JsonString, // JSONB
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
    pub settings: JsonString, // JSONB
    pub tags: JsonString, // JSONB
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
    pub settings: JsonString, // JSONB
    pub tags: JsonString, // JSONB
}

/// Quality profile database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QualityProfileDbModel {
    pub id: i64,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: i32,
    pub items: JsonString, // JSONB
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub format_items: JsonString, // JSONB
}

/// Language profile database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LanguageProfileDbModel {
    pub id: i64,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: i32,
    pub languages: JsonString, // JSONB
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
    pub tags: JsonString, // JSONB
    pub order: i32,
}

/// Custom format database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomFormatDbModel {
    pub id: i64,
    pub name: String,
    pub include_custom_format_when_renaming: bool,
    pub specifications: JsonString, // JSONB
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
    pub quality: JsonString, // JSONB
    pub languages: JsonString, // JSONB
    pub custom_formats: JsonString, // JSONB
    pub custom_format_score: i32,
    pub quality_cutoff_not_met: bool,
    pub date: DateTime<Utc>,
    pub download_id: Option<String>,
    pub event_type: i32,
    pub data: JsonString, // JSONB
}

/// Blocklist database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlocklistDbModel {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: JsonString, // JSONB
    pub source_title: String,
    pub quality: JsonString, // JSONB
    pub languages: JsonString, // JSONB
    pub custom_formats: JsonString, // JSONB
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
    pub filters: JsonString, // JSONB
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
    pub unmapped_folders: Option<JsonString>, // JSONB
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

// TrackedDownloadDbModel removed in v0.102.0 — tracked downloads now use
// per-content-type JSONL flat files (see core/queue/tracked.rs).
// The `tracked_downloads` DB table is still read by stores.rs for one-time migration.

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
    pub tags: JsonString, // JSONB
}

/// Release profile database model
/// Preferred words, required/ignored terms for release scoring
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReleaseProfileDbModel {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub required: JsonString, // JSONB array of required terms
    pub ignored: JsonString, // JSONB array of ignored terms
    pub preferred: JsonString, // JSONB array of {key: term, value: score}
    pub include_preferred_when_renaming: bool,
    pub indexer_id: i64,
    pub tags: JsonString, // JSONB
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
    pub genres: JsonString, // JSONB
    pub images: JsonString, // JSONB
    pub tags: JsonString, // JSONB
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
    pub secondary_types: JsonString, // JSONB array of secondary types
    pub release_date: Option<NaiveDate>,
    pub genres: JsonString, // JSONB
    pub images: JsonString, // JSONB
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
    pub quality: JsonString, // JSONB
    pub media_info: Option<JsonString>, // JSONB
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
    pub genres: JsonString, // JSONB
    pub images: JsonString, // JSONB
    pub tags: JsonString, // JSONB
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
    pub quality: JsonString, // JSONB
    pub media_info: Option<JsonString>, // JSONB
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
    pub genres: JsonString, // JSONB
    pub images: JsonString, // JSONB
    pub tags: JsonString, // JSONB
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
    pub quality: JsonString, // JSONB
    pub media_info: Option<JsonString>, // JSONB
    pub date_added: DateTime<Utc>,
}
