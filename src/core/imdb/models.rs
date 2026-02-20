//! IMDB data models

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// IMDB Series (TV Show) from title.basics.tsv.gz
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbSeries {
    /// Numeric IMDB ID (e.g., 10234724 from tt10234724)
    pub imdb_id: i64,
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
    /// Comma-separated genres
    pub genres: Option<String>,
    /// Is adult content
    pub is_adult: bool,
    /// Type: tvSeries, tvMiniSeries, etc.
    pub title_type: String,
    /// Average rating (1-10)
    pub rating: Option<f32>,
    /// Number of votes
    pub votes: Option<i32>,
    /// When this record was last synced
    pub last_synced_at: DateTime<Utc>,
}

impl ImdbSeries {
    /// Get genres as a vector
    pub fn genres_vec(&self) -> Vec<&str> {
        self.genres
            .as_ref()
            .map(|g| g.split(',').collect())
            .unwrap_or_default()
    }

    /// Get the full IMDB URL for this series
    pub fn imdb_url(&self) -> String {
        format!("https://www.imdb.com/title/tt{:07}/", self.imdb_id)
    }

    /// Check if this series is still ongoing
    pub fn is_ongoing(&self) -> bool {
        self.end_year.is_none()
    }
}

/// IMDB Episode from title.episode.tsv.gz joined with title.basics.tsv.gz
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbEpisode {
    /// Episode's own IMDB ID (numeric)
    pub imdb_id: i64,
    /// Parent series IMDB ID (numeric)
    pub parent_imdb_id: i64,
    /// Season number (may be None for specials)
    pub season_number: Option<i32>,
    /// Episode number within the season
    pub episode_number: Option<i32>,
    /// Episode title
    pub title: Option<String>,
    /// Runtime in minutes
    pub runtime_minutes: Option<i32>,
    /// Average rating (1-10)
    pub rating: Option<f32>,
    /// Number of votes
    pub votes: Option<i32>,
    /// Air date (NOT from IMDB - populated from TMDB/TVMaze)
    pub air_date: Option<NaiveDate>,
    /// When this record was last synced
    pub last_synced_at: DateTime<Utc>,
}

impl ImdbEpisode {
    /// Get the full IMDB URL for this episode
    pub fn imdb_url(&self) -> String {
        format!("https://www.imdb.com/title/tt{:07}/", self.imdb_id)
    }

    /// Check if this is a special (no season/episode number)
    pub fn is_special(&self) -> bool {
        self.season_number.is_none() || self.season_number == Some(0)
    }
}

/// IMDB Alternative Title from title.akas.tsv.gz
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbAka {
    /// Database ID
    pub id: Option<i64>,
    /// Title IMDB ID (numeric)
    pub imdb_id: i64,
    /// Ordering (for sorting multiple AKAs)
    pub ordering: i32,
    /// Alternative title
    pub title: String,
    /// Region code (e.g., "US", "GB", "JP")
    pub region: Option<String>,
    /// Language code
    pub language: Option<String>,
    /// Type flags (e.g., "imdbDisplay", "original")
    pub types: Option<String>,
    /// Additional attributes
    pub attributes: Option<String>,
    /// Is this the original title
    pub is_original_title: bool,
}

/// IMDB Person from name.basics.tsv.gz
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbPerson {
    /// Numeric IMDB ID (e.g., 123456 from nm0123456)
    pub imdb_id: i64,
    /// Primary name
    pub name: String,
    /// Birth year
    pub birth_year: Option<i32>,
    /// Death year (None if alive or unknown)
    pub death_year: Option<i32>,
    /// Comma-separated primary professions
    pub primary_profession: Option<String>,
    /// Comma-separated known-for title IDs (numeric)
    pub known_for_titles: Option<String>,
    /// When this record was last synced
    pub last_synced_at: DateTime<Utc>,
}

impl ImdbPerson {
    /// Get the full IMDB URL for this person
    pub fn imdb_url(&self) -> String {
        format!("https://www.imdb.com/name/nm{:07}/", self.imdb_id)
    }

    /// Get professions as a vector
    pub fn professions_vec(&self) -> Vec<&str> {
        self.primary_profession
            .as_ref()
            .map(|p| p.split(',').collect())
            .unwrap_or_default()
    }
}

/// IMDB Crew from title.crew.tsv.gz
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbCrew {
    /// Title IMDB ID (numeric)
    pub imdb_id: i64,
    /// Comma-separated director IMDB IDs (numeric, without nm prefix)
    pub directors: Option<String>,
    /// Comma-separated writer IMDB IDs (numeric, without nm prefix)
    pub writers: Option<String>,
}

/// Sync status for tracking IMDB dataset imports
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbSyncStatus {
    pub id: Option<i64>,
    /// Dataset name (e.g., "title.basics")
    pub dataset_name: String,
    /// Date from the dataset file
    pub file_date: Option<String>,
    /// Total rows processed
    pub rows_processed: i64,
    /// Rows inserted
    pub rows_inserted: i64,
    /// Rows updated
    pub rows_updated: i64,
    /// When sync started
    pub started_at: DateTime<Utc>,
    /// When sync completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Status: running, completed, failed
    pub status: String,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Last successfully processed IMDB ID (for resumability)
    pub last_processed_id: Option<i64>,
}

/// A parsed row from title.basics.tsv.gz
#[derive(Debug)]
pub struct TitleBasicsRow {
    pub tconst: i64,
    pub title_type: String,
    pub primary_title: String,
    pub original_title: String,
    pub is_adult: bool,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
    pub runtime_minutes: Option<i32>,
    pub genres: Option<String>,
}

/// A parsed row from title.episode.tsv.gz
#[derive(Debug)]
pub struct TitleEpisodeRow {
    pub tconst: i64,
    pub parent_tconst: i64,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
}

/// A parsed row from title.ratings.tsv.gz
#[derive(Debug)]
pub struct TitleRatingsRow {
    pub tconst: i64,
    pub average_rating: f32,
    pub num_votes: i32,
}

/// Title types we care about for TV content
pub const TV_TITLE_TYPES: &[&str] = &[
    "tvSeries",
    "tvMiniSeries",
    "tvMovie",
    "tvSpecial",
    "tvShort",
];

/// Check if a title type is a TV series (what we want to import)
pub fn is_tv_series(title_type: &str) -> bool {
    title_type == "tvSeries" || title_type == "tvMiniSeries"
}
