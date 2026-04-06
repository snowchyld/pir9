//! Data models for the IMDB service

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// IMDB Series (TV Show)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbSeries {
    /// IMDB ID as string (e.g., "tt10234724")
    pub imdb_id: String,
    /// Primary title
    pub title: String,
    /// Original title (may differ from primary for non-English content)
    pub original_title: Option<String>,
    /// Year the series started
    pub start_year: Option<i32>,
    /// Year the series ended (None if ongoing)
    pub end_year: Option<i32>,
    /// Runtime in minutes per episode
    pub runtime_minutes: Option<i32>,
    /// Genres as a vector
    pub genres: Vec<String>,
    /// Is adult content
    pub is_adult: bool,
    /// Average rating (1-10)
    pub rating: Option<f64>,
    /// Number of votes
    pub votes: Option<i64>,
}

/// IMDB Episode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbEpisode {
    /// Episode's own IMDB ID
    pub imdb_id: String,
    /// Parent series IMDB ID
    pub parent_imdb_id: String,
    /// Season number
    pub season_number: Option<i32>,
    /// Episode number within the season
    pub episode_number: Option<i32>,
    /// Episode title
    pub title: Option<String>,
    /// Runtime in minutes
    pub runtime_minutes: Option<i32>,
    /// Average rating (1-10)
    pub rating: Option<f64>,
    /// Number of votes
    pub votes: Option<i64>,
    /// Air date
    pub air_date: Option<String>,
}

/// IMDB Movie
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbMovie {
    /// IMDB ID as string (e.g., "tt0133093")
    pub imdb_id: String,
    /// Primary title
    pub title: String,
    /// Original title (may differ from primary for non-English content)
    pub original_title: Option<String>,
    /// Release year
    pub year: Option<i32>,
    /// Runtime in minutes
    pub runtime_minutes: Option<i32>,
    /// Genres as a vector
    pub genres: Vec<String>,
    /// Is adult content
    pub is_adult: bool,
    /// Average rating (1-10)
    pub rating: Option<f64>,
    /// Number of votes
    pub votes: Option<i64>,
    /// TMDB ID (cached from on-demand TMDB lookup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb_id: Option<i64>,
    /// Poster image URL (cached from TMDB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    /// Fanart/backdrop image URL (cached from TMDB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fanart_url: Option<String>,
}

/// Service statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbStats {
    pub series_count: i64,
    pub episode_count: i64,
    pub movie_count: i64,
    pub people_count: i64,
    pub credits_count: i64,
    pub last_sync: Option<String>,
    pub db_size_bytes: Option<i64>,
}

/// Sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// Whether any sync task is currently running (from in-memory handle, not DB)
    #[serde(default)]
    pub is_running: bool,
    pub title_basics: Option<DatasetSyncStatus>,
    pub title_episodes: Option<DatasetSyncStatus>,
    pub title_ratings: Option<DatasetSyncStatus>,
    pub name_basics: Option<DatasetSyncStatus>,
    pub title_principals: Option<DatasetSyncStatus>,
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
    /// Which file is currently being downloaded (e.g. "title.basics.tsv.gz")
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
    pub title_basics: Option<SyncStats>,
    pub title_episodes: Option<SyncStats>,
    pub title_ratings: Option<SyncStats>,
    pub name_basics: Option<SyncStats>,
    pub title_principals: Option<SyncStats>,
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

/// Internal database row for series
#[derive(Debug, Clone)]
pub struct DbSeries {
    pub imdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
    pub runtime_minutes: Option<i32>,
    pub genres: Option<String>,
    pub is_adult: bool,
    pub title_type: String,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbSeries {
    /// Convert to API response format
    pub fn to_api(&self) -> ImdbSeries {
        ImdbSeries {
            imdb_id: format!("tt{:07}", self.imdb_id),
            title: self.title.clone(),
            original_title: self.original_title.clone(),
            start_year: self.start_year,
            end_year: self.end_year,
            runtime_minutes: self.runtime_minutes,
            genres: self
                .genres
                .as_ref()
                .map(|g| g.split(',').map(String::from).collect())
                .unwrap_or_default(),
            is_adult: self.is_adult,
            rating: self.rating,
            votes: self.votes,
        }
    }
}

/// Internal database row for episode
#[derive(Debug, Clone)]
pub struct DbEpisode {
    pub imdb_id: i64,
    pub parent_imdb_id: i64,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub title: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub air_date: Option<NaiveDate>,
    pub last_synced_at: DateTime<Utc>,
}

impl DbEpisode {
    /// Convert to API response format
    pub fn to_api(&self) -> ImdbEpisode {
        ImdbEpisode {
            imdb_id: format!("tt{:07}", self.imdb_id),
            parent_imdb_id: format!("tt{:07}", self.parent_imdb_id),
            season_number: self.season_number,
            episode_number: self.episode_number,
            title: self.title.clone(),
            runtime_minutes: self.runtime_minutes,
            rating: self.rating,
            votes: self.votes,
            air_date: self.air_date.map(|d| d.to_string()),
        }
    }
}

/// Internal database row for movie
#[derive(Debug, Clone)]
pub struct DbMovie {
    pub imdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub runtime_minutes: Option<i32>,
    pub genres: Option<String>,
    pub is_adult: bool,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub last_synced_at: DateTime<Utc>,
    pub tmdb_id: Option<i64>,
    pub poster_url: Option<String>,
    pub fanart_url: Option<String>,
    pub tmdb_fetched_at: Option<DateTime<Utc>>,
}

impl DbMovie {
    /// Convert to API response format
    pub fn to_api(&self) -> ImdbMovie {
        ImdbMovie {
            imdb_id: format!("tt{:07}", self.imdb_id),
            title: self.title.clone(),
            original_title: self.original_title.clone(),
            year: self.year,
            runtime_minutes: self.runtime_minutes,
            genres: self
                .genres
                .as_ref()
                .map(|g| g.split(',').map(String::from).collect())
                .unwrap_or_default(),
            is_adult: self.is_adult,
            rating: self.rating,
            votes: self.votes,
            tmdb_id: self.tmdb_id,
            poster_url: self.poster_url.clone(),
            fanart_url: self.fanart_url.clone(),
        }
    }
}

/// Internal database row for a person (from name.basics)
#[derive(Debug, Clone)]
pub struct DbPerson {
    pub nconst: i64,
    pub primary_name: String,
    pub birth_year: Option<i16>,
    pub death_year: Option<i16>,
    pub primary_profession: Option<String>,
    pub known_for_titles: Option<String>,
}

/// Internal database row for a credit (from title.principals)
#[derive(Debug, Clone)]
pub struct DbCredit {
    pub tconst: i64,
    pub nconst: i64,
    pub ordering: i16,
    pub category: String,
    pub job: Option<String>,
    pub characters: Option<String>,
}

/// API response: a single credit entry with resolved person name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbCredit {
    pub nconst: String,
    pub name: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters: Option<Vec<String>>,
    pub ordering: i16,
}

/// API response: all credits for a title
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleCredits {
    pub imdb_id: String,
    pub credits: Vec<ImdbCredit>,
}

/// API response: person details with filmography
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbPerson {
    pub nconst: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_year: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub death_year: Option<i16>,
    pub professions: Vec<String>,
    pub known_for: Vec<String>,
}

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
    /// Filename (e.g. "title.basics.tsv.gz")
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

/// Parse IMDB ID string to numeric (e.g., "tt10234724" -> 10234724)
pub fn parse_imdb_id(id_str: &str) -> Option<i64> {
    if id_str.len() > 2 && id_str.starts_with("tt") {
        id_str[2..].parse().ok()
    } else {
        id_str.parse().ok()
    }
}

/// Parse IMDB person ID to numeric (e.g., "nm0000001" -> 1)
pub fn parse_nconst(id_str: &str) -> Option<i64> {
    if id_str.len() > 2 && id_str.starts_with("nm") {
        id_str[2..].parse().ok()
    } else {
        id_str.parse().ok()
    }
}
