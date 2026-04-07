//! Data models for the MusicBrainz service

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// API response types
// ============================================================================

/// MusicBrainz Artist (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbArtist {
    pub mbid: String,
    pub name: String,
    pub sort_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating_count: Option<i32>,
}

/// MusicBrainz Release Group (≈ Album) (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbReleaseGroup {
    pub mbid: String,
    pub title: String,
    pub artist_mbid: String,
    pub artist_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_type: Option<String>,
    pub secondary_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_release_date: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art_url: Option<String>,
}

/// MusicBrainz Release (specific edition) (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbRelease {
    pub mbid: String,
    pub release_group_mbid: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub track_count: i32,
}

/// MusicBrainz Release Track (track listing entry) (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbReleaseTrack {
    pub disc_number: i32,
    pub position: i32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_mbid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_ms: Option<i32>,
}

/// Internal database row for release track
#[derive(Debug, Clone)]
pub struct DbReleaseTrack {
    pub release_mbid: String,
    pub disc_number: i32,
    pub position: i32,
    pub title: String,
    pub recording_mbid: Option<String>,
    pub length_ms: Option<i32>,
}

impl DbReleaseTrack {
    pub fn to_api(&self) -> MbReleaseTrack {
        MbReleaseTrack {
            disc_number: self.disc_number,
            position: self.position,
            title: self.title.clone(),
            recording_mbid: self.recording_mbid.clone(),
            length_ms: self.length_ms,
        }
    }
}

/// MusicBrainz Label (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbLabel {
    pub mbid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
}

/// MusicBrainz Recording (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbRecording {
    pub mbid: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_mbid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_release_date: Option<String>,
    pub isrcs: Vec<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
}

/// MusicBrainz Work (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbWork {
    pub mbid: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_type: Option<String>,
    pub languages: Vec<String>,
    pub iswcs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
}

/// MusicBrainz Area (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbArea {
    pub mbid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso_3166_1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso_3166_2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
}

/// MusicBrainz Series (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbSeries {
    pub mbid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
}

/// MusicBrainz Event (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbEvent {
    pub mbid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
}

/// MusicBrainz Instrument (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbInstrument {
    pub mbid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
}

/// MusicBrainz Place (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbPlace {
    pub mbid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
}

/// Service statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbStats {
    pub artist_count: i64,
    pub release_group_count: i64,
    pub release_count: i64,
    pub cover_art_count: i64,
    pub label_count: i64,
    pub recording_count: i64,
    pub work_count: i64,
    pub area_count: i64,
    pub series_count: i64,
    pub event_count: i64,
    pub instrument_count: i64,
    pub place_count: i64,
    pub last_sync: Option<String>,
    pub db_size_bytes: Option<i64>,
}

/// Sync status (all datasets)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// Whether any sync task is currently running (from in-memory handle, not DB)
    #[serde(default)]
    pub is_running: bool,
    pub artists: Option<DatasetSyncStatus>,
    pub release_groups: Option<DatasetSyncStatus>,
    pub releases: Option<DatasetSyncStatus>,
    pub labels: Option<DatasetSyncStatus>,
    pub recordings: Option<DatasetSyncStatus>,
    pub works: Option<DatasetSyncStatus>,
    pub areas: Option<DatasetSyncStatus>,
    pub series: Option<DatasetSyncStatus>,
    pub events: Option<DatasetSyncStatus>,
    pub instruments: Option<DatasetSyncStatus>,
    pub places: Option<DatasetSyncStatus>,
}

/// Status for a single dataset sync
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSyncStatus {
    pub dataset_name: String,
    pub rows_processed: i64,
    pub rows_inserted: i64,
    pub rows_updated: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    #[serde(default)]
    pub is_running: bool,
    /// Download progress: 0.0 to 100.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_progress: Option<f64>,
    /// Total file size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_size_bytes: Option<u64>,
    /// Bytes downloaded so far
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_bytes_done: Option<u64>,
    /// Current phase: "downloading", "parsing", "idle"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
}

/// Live download progress state shared between sync task and status API
#[derive(Debug, Clone, Default)]
pub struct DownloadProgress {
    /// Which file is currently being downloaded (e.g. "artist.tar.xz")
    pub current_file: String,
    /// Current phase: "downloading", "parsing", "idle"
    pub phase: String,
    /// Download percentage: 0.0 to 100.0
    pub percentage: f64,
    /// Bytes downloaded so far
    pub bytes_done: u64,
    /// Total bytes expected
    pub total_bytes: u64,
    /// When true, skip downloads and use cached files as-is
    pub process_only: bool,
}

/// Sync report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub artists: Option<SyncStats>,
    pub release_groups: Option<SyncStats>,
    pub releases: Option<SyncStats>,
    pub labels: Option<SyncStats>,
    pub recordings: Option<SyncStats>,
    pub works: Option<SyncStats>,
    pub areas: Option<SyncStats>,
    pub series: Option<SyncStats>,
    pub events: Option<SyncStats>,
    pub instruments: Option<SyncStats>,
    pub places: Option<SyncStats>,
    pub errors: Vec<String>,
}

/// Stats from a single dataset sync
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStats {
    pub rows_processed: i64,
    pub rows_inserted: i64,
    pub rows_updated: i64,
    pub duration_seconds: i64,
}

// ============================================================================
// Internal DB row types
// ============================================================================

/// Internal database row for artist
#[derive(Debug, Clone)]
pub struct DbArtist {
    pub mbid: String,
    pub name: String,
    pub sort_name: String,
    pub artist_type: Option<String>,
    pub gender: Option<String>,
    pub area: Option<String>,
    pub begin_date: Option<String>,
    pub end_date: Option<String>,
    pub disambiguation: Option<String>,
    pub genres: String,
    pub tags: String,
    pub rating: Option<f64>,
    pub rating_count: Option<i32>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbArtist {
    /// Convert to API response format
    pub fn to_api(&self) -> MbArtist {
        MbArtist {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            sort_name: self.sort_name.clone(),
            artist_type: self.artist_type.clone(),
            gender: self.gender.clone(),
            area: self.area.clone(),
            begin_date: self.begin_date.clone(),
            end_date: self.end_date.clone(),
            disambiguation: self.disambiguation.clone(),
            genres: parse_json_string_array(&self.genres),
            tags: parse_json_string_array(&self.tags),
            rating: self.rating,
            rating_count: self.rating_count,
        }
    }
}

/// Internal database row for release group
#[derive(Debug, Clone)]
pub struct DbReleaseGroup {
    pub mbid: String,
    pub title: String,
    pub artist_mbid: String,
    pub artist_name: String,
    pub primary_type: Option<String>,
    pub secondary_types: String,
    pub first_release_date: Option<String>,
    pub genres: String,
    pub tags: String,
    pub rating: Option<f64>,
    pub rating_count: Option<i32>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbReleaseGroup {
    /// Convert to API response format
    pub fn to_api(&self) -> MbReleaseGroup {
        MbReleaseGroup {
            mbid: self.mbid.clone(),
            title: self.title.clone(),
            artist_mbid: self.artist_mbid.clone(),
            artist_name: self.artist_name.clone(),
            primary_type: self.primary_type.clone(),
            secondary_types: parse_json_string_array(&self.secondary_types),
            first_release_date: self.first_release_date.clone(),
            genres: parse_json_string_array(&self.genres),
            tags: parse_json_string_array(&self.tags),
            rating: self.rating,
            rating_count: self.rating_count,
            cover_art_url: None, // Enriched by handler from mb_cover_art table
        }
    }
}

/// Internal database row for release
#[derive(Debug, Clone)]
pub struct DbRelease {
    pub mbid: String,
    pub release_group_mbid: String,
    pub title: String,
    pub artist_mbid: Option<String>,
    pub artist_name: Option<String>,
    pub date: Option<String>,
    pub country: Option<String>,
    pub status: Option<String>,
    pub barcode: Option<String>,
    pub packaging: Option<String>,
    pub track_count: i32,
    pub last_synced_at: DateTime<Utc>,
}

impl DbRelease {
    /// Convert to API response format
    pub fn to_api(&self) -> MbRelease {
        MbRelease {
            mbid: self.mbid.clone(),
            release_group_mbid: self.release_group_mbid.clone(),
            title: self.title.clone(),
            date: self.date.clone(),
            country: self.country.clone(),
            status: self.status.clone(),
            track_count: self.track_count,
        }
    }
}

/// Internal database row for label
#[derive(Debug, Clone)]
pub struct DbLabel {
    pub mbid: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub label_type: Option<String>,
    pub area: Option<String>,
    pub label_code: Option<i32>,
    pub begin_date: Option<String>,
    pub end_date: Option<String>,
    pub disambiguation: Option<String>,
    pub genres: String,
    pub tags: String,
    pub last_synced_at: DateTime<Utc>,
}

impl DbLabel {
    pub fn to_api(&self) -> MbLabel {
        MbLabel {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            sort_name: self.sort_name.clone(),
            label_type: self.label_type.clone(),
            area: self.area.clone(),
            label_code: self.label_code,
            begin_date: self.begin_date.clone(),
            end_date: self.end_date.clone(),
            disambiguation: self.disambiguation.clone(),
            genres: parse_json_string_array(&self.genres),
            tags: parse_json_string_array(&self.tags),
        }
    }
}

/// Internal database row for recording
#[derive(Debug, Clone)]
pub struct DbRecording {
    pub mbid: String,
    pub title: String,
    pub artist_mbid: Option<String>,
    pub artist_name: Option<String>,
    pub length_ms: Option<i32>,
    pub first_release_date: Option<String>,
    pub isrcs: String,
    pub genres: String,
    pub tags: String,
    pub last_synced_at: DateTime<Utc>,
}

impl DbRecording {
    pub fn to_api(&self) -> MbRecording {
        MbRecording {
            mbid: self.mbid.clone(),
            title: self.title.clone(),
            artist_mbid: self.artist_mbid.clone(),
            artist_name: self.artist_name.clone(),
            length_ms: self.length_ms,
            first_release_date: self.first_release_date.clone(),
            isrcs: parse_json_string_array(&self.isrcs),
            genres: parse_json_string_array(&self.genres),
            tags: parse_json_string_array(&self.tags),
        }
    }
}

/// Internal database row for work
#[derive(Debug, Clone)]
pub struct DbWork {
    pub mbid: String,
    pub title: String,
    pub work_type: Option<String>,
    pub languages: String,
    pub iswcs: String,
    pub disambiguation: Option<String>,
    pub genres: String,
    pub tags: String,
    pub last_synced_at: DateTime<Utc>,
}

impl DbWork {
    pub fn to_api(&self) -> MbWork {
        MbWork {
            mbid: self.mbid.clone(),
            title: self.title.clone(),
            work_type: self.work_type.clone(),
            languages: parse_json_string_array(&self.languages),
            iswcs: parse_json_string_array(&self.iswcs),
            disambiguation: self.disambiguation.clone(),
            genres: parse_json_string_array(&self.genres),
            tags: parse_json_string_array(&self.tags),
        }
    }
}

/// Internal database row for area
#[derive(Debug, Clone)]
pub struct DbArea {
    pub mbid: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub area_type: Option<String>,
    pub iso_3166_1: Option<String>,
    pub iso_3166_2: Option<String>,
    pub disambiguation: Option<String>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbArea {
    pub fn to_api(&self) -> MbArea {
        MbArea {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            sort_name: self.sort_name.clone(),
            area_type: self.area_type.clone(),
            iso_3166_1: self.iso_3166_1.clone(),
            iso_3166_2: self.iso_3166_2.clone(),
            disambiguation: self.disambiguation.clone(),
        }
    }
}

/// Internal database row for series
#[derive(Debug, Clone)]
pub struct DbSeries {
    pub mbid: String,
    pub name: String,
    pub series_type: Option<String>,
    pub disambiguation: Option<String>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbSeries {
    pub fn to_api(&self) -> MbSeries {
        MbSeries {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            series_type: self.series_type.clone(),
            disambiguation: self.disambiguation.clone(),
        }
    }
}

/// Internal database row for event
#[derive(Debug, Clone)]
pub struct DbEvent {
    pub mbid: String,
    pub name: String,
    pub event_type: Option<String>,
    pub begin_date: Option<String>,
    pub end_date: Option<String>,
    pub time: Option<String>,
    pub cancelled: bool,
    pub disambiguation: Option<String>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbEvent {
    pub fn to_api(&self) -> MbEvent {
        MbEvent {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            event_type: self.event_type.clone(),
            begin_date: self.begin_date.clone(),
            end_date: self.end_date.clone(),
            time: self.time.clone(),
            cancelled: self.cancelled,
            disambiguation: self.disambiguation.clone(),
        }
    }
}

/// Internal database row for instrument
#[derive(Debug, Clone)]
pub struct DbInstrument {
    pub mbid: String,
    pub name: String,
    pub instrument_type: Option<String>,
    pub description: Option<String>,
    pub disambiguation: Option<String>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbInstrument {
    pub fn to_api(&self) -> MbInstrument {
        MbInstrument {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            instrument_type: self.instrument_type.clone(),
            description: self.description.clone(),
            disambiguation: self.disambiguation.clone(),
        }
    }
}

/// Internal database row for place
#[derive(Debug, Clone)]
pub struct DbPlace {
    pub mbid: String,
    pub name: String,
    pub place_type: Option<String>,
    pub area: Option<String>,
    pub coordinates: Option<String>,
    pub disambiguation: Option<String>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbPlace {
    pub fn to_api(&self) -> MbPlace {
        MbPlace {
            mbid: self.mbid.clone(),
            name: self.name.clone(),
            place_type: self.place_type.clone(),
            area: self.area.clone(),
            coordinates: self.coordinates.clone(),
            disambiguation: self.disambiguation.clone(),
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Request body for selective sync/download/process
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    /// Which datasets to operate on. Empty = all.
    #[serde(default)]
    pub datasets: Vec<String>,
}

/// Metadata about a single dataset file (for GET /api/datasets)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo {
    /// Filename (e.g. "artist.tar.xz")
    pub name: String,
    /// Remote file size in bytes (from HTTP HEAD)
    pub remote_size: Option<u64>,
    /// Local cached file size in bytes (None if not cached)
    pub local_size: Option<u64>,
    /// Human-readable age of local cache (e.g. "2h 15m")
    pub local_age: Option<String>,
    /// Whether a usable cached copy exists
    pub cached: bool,
}

/// Parse a JSON array string (e.g., '["rock","pop"]') into a Vec<String>
fn parse_json_string_array(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}
