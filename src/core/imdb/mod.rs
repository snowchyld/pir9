//! IMDB Non-Commercial Dataset Sync Service
//!
//! This module provides functionality to sync data from IMDB's non-commercial datasets:
//! https://datasets.imdbws.com/
//!
//! The datasets are TSV files updated daily, containing:
//! - title.basics.tsv.gz - Basic title info (series, movies)
//! - title.episode.tsv.gz - Episode to series mappings
//! - title.ratings.tsv.gz - Ratings and vote counts
//! - title.akas.tsv.gz - Alternative titles/translations
//! - title.crew.tsv.gz - Directors and writers
//! - name.basics.tsv.gz - People (actors, directors, etc.)
//!
//! Note: IMDB datasets do NOT include:
//! - Episode air dates (must come from TMDB/TVMaze)
//! - Images/artwork
//! - Plot summaries

pub mod database;
pub mod models;
pub mod repository;
pub mod sync;
pub mod tvmaze;

pub use database::{ImdbClient, ImdbDatabase, ImdbProxyResponse, DEFAULT_IMDB_DB_PATH};
pub use models::*;
pub use repository::ImdbRepository;
pub use sync::ImdbSyncService;
pub use tvmaze::TvMazeService;

/// IMDB dataset base URL
pub const IMDB_DATASETS_URL: &str = "https://datasets.imdbws.com";

/// Available IMDB datasets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImdbDataset {
    TitleBasics,
    TitleEpisode,
    TitleRatings,
    TitleAkas,
    TitleCrew,
    NameBasics,
}

impl ImdbDataset {
    /// Get the filename for this dataset
    pub fn filename(&self) -> &'static str {
        match self {
            Self::TitleBasics => "title.basics.tsv.gz",
            Self::TitleEpisode => "title.episode.tsv.gz",
            Self::TitleRatings => "title.ratings.tsv.gz",
            Self::TitleAkas => "title.akas.tsv.gz",
            Self::TitleCrew => "title.crew.tsv.gz",
            Self::NameBasics => "name.basics.tsv.gz",
        }
    }

    /// Get the full URL for this dataset
    pub fn url(&self) -> String {
        format!("{}/{}", IMDB_DATASETS_URL, self.filename())
    }

    /// Get the database table name for this dataset
    pub fn table_name(&self) -> &'static str {
        match self {
            Self::TitleBasics => "imdb_series",
            Self::TitleEpisode => "imdb_episodes",
            Self::TitleRatings => "imdb_series", // Updates ratings in series table
            Self::TitleAkas => "imdb_akas",
            Self::TitleCrew => "imdb_crew",
            Self::NameBasics => "imdb_people",
        }
    }
}

/// Parse an IMDB ID string (e.g., "tt10234724" or "nm0000123") to its numeric value
pub fn parse_imdb_id(id_str: &str) -> Option<i64> {
    // Handle \N (null) values from IMDB
    if id_str == "\\N" || id_str.is_empty() {
        return None;
    }

    // Strip the prefix (tt, nm, co, etc.) and parse the number
    let numeric_part = if id_str.len() > 2 && id_str.starts_with(|c: char| c.is_ascii_alphabetic()) {
        &id_str[2..]
    } else {
        id_str
    };

    numeric_part.parse().ok()
}

/// Format a numeric IMDB ID back to its string representation
pub fn format_imdb_id(id: i64, prefix: &str) -> String {
    format!("{}{:07}", prefix, id)
}

/// Format a title IMDB ID (tt prefix)
pub fn format_title_id(id: i64) -> String {
    format_imdb_id(id, "tt")
}

/// Format a name IMDB ID (nm prefix)
pub fn format_name_id(id: i64) -> String {
    format_imdb_id(id, "nm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_imdb_id() {
        assert_eq!(parse_imdb_id("tt10234724"), Some(10234724));
        assert_eq!(parse_imdb_id("nm0000123"), Some(123));
        assert_eq!(parse_imdb_id("\\N"), None);
        assert_eq!(parse_imdb_id(""), None);
    }

    #[test]
    fn test_format_imdb_id() {
        assert_eq!(format_title_id(10234724), "tt10234724");
        assert_eq!(format_title_id(123), "tt0000123");
        assert_eq!(format_name_id(123), "nm0000123");
    }
}
