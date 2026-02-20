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
}

/// Service statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbStats {
    pub series_count: i64,
    pub episode_count: i64,
    pub movie_count: i64,
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
}

/// Sync report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub title_basics: Option<SyncStats>,
    pub title_episodes: Option<SyncStats>,
    pub title_ratings: Option<SyncStats>,
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
        }
    }
}

/// Parse IMDB ID string to numeric (e.g., "tt10234724" -> 10234724)
pub fn parse_imdb_id(id_str: &str) -> Option<i64> {
    if id_str.len() > 2 && id_str.starts_with("tt") {
        id_str[2..].parse().ok()
    } else {
        id_str.parse().ok()
    }
}
