#![allow(dead_code, unused_imports)]
//! TV Show domain models
//! Series, Episode, and related entities

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::core::profiles::languages::Language;
use crate::core::profiles::QualityProfile;
use crate::core::tv::{
    Actor, MediaCover, NewItemMonitorType, Ratings, Season, SeriesStatusType, SeriesType,
};

/// Series entity - represents a TV show
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub id: i64,

    // External IDs
    pub tvdb_id: i64,
    pub tv_rage_id: i64,
    pub tv_maze_id: i64,
    pub imdb_id: Option<String>,
    pub tmdb_id: i64,
    pub mal_ids: HashSet<i64>,
    pub anilist_ids: HashSet<i64>,

    // Basic info
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub status: SeriesStatusType,
    pub overview: Option<String>,
    pub air_time: Option<NaiveTime>,
    pub monitored: bool,
    pub monitor_new_items: NewItemMonitorType,

    // Quality and profile
    pub quality_profile_id: i64,
    pub quality_profile: Option<QualityProfile>,
    pub language_profile_id: Option<i64>,

    // Organization
    pub season_folder: bool,
    pub series_type: SeriesType,
    pub title_slug: String,
    pub path: String,
    pub root_folder_path: String,

    // Metadata
    pub year: i32,
    pub first_aired: Option<NaiveDate>,
    pub last_aired: Option<NaiveDate>,
    pub runtime: i32,
    pub network: Option<String>,
    pub certification: Option<String>,
    pub genres: Vec<String>,
    pub actors: Vec<Actor>,
    pub images: Vec<MediaCover>,
    pub ratings: Option<Ratings>,
    pub use_scene_numbering: bool,
    pub episode_ordering: String,

    // Seasons
    pub seasons: Vec<Season>,

    // Tags
    pub tags: HashSet<i64>,

    // Statistics
    pub statistics: Option<SeriesStatistics>,

    // Timestamps
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

impl Series {
    /// Create a new series with default values
    pub fn new(title: String, tvdb_id: i64) -> Self {
        let clean_title = Self::clean_title(&title);
        let sort_title = clean_title.clone();
        let title_slug = Self::generate_slug(&title);

        Self {
            id: 0,
            tvdb_id,
            tv_rage_id: 0,
            tv_maze_id: 0,
            imdb_id: None,
            tmdb_id: 0,
            mal_ids: HashSet::new(),
            anilist_ids: HashSet::new(),
            title,
            clean_title,
            sort_title,
            title_slug,
            status: SeriesStatusType::Continuing,
            overview: None,
            air_time: None,
            monitored: true,
            monitor_new_items: NewItemMonitorType::All,
            quality_profile_id: 0,
            quality_profile: None,
            language_profile_id: None,
            season_folder: true,
            series_type: SeriesType::Standard,
            path: String::new(),
            root_folder_path: String::new(),
            year: 0,
            first_aired: None,
            last_aired: None,
            runtime: 0,
            network: None,
            certification: None,
            genres: Vec::new(),
            actors: Vec::new(),
            images: Vec::new(),
            ratings: None,
            use_scene_numbering: false,
            episode_ordering: "aired".to_string(),
            seasons: Vec::new(),
            tags: HashSet::new(),
            statistics: None,
            added: Utc::now(),
            last_info_sync: None,
        }
    }

    /// Clean a title for searching/sorting
    fn clean_title(title: &str) -> String {
        title
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate URL-friendly slug
    fn generate_slug(title: &str) -> String {
        title
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
            .replace(' ', "-")
            .replace("--", "-")
            .trim_matches('-')
            .to_string()
    }

    /// Check if series needs refresh from metadata source
    pub fn needs_refresh(&self) -> bool {
        match self.last_info_sync {
            None => true,
            Some(last_sync) => {
                let elapsed = Utc::now() - last_sync;
                elapsed.num_hours() > 24
            }
        }
    }

    /// Get the full path for this series
    pub fn full_path(&self) -> String {
        format!(
            "{}/{}",
            self.root_folder_path.trim_end_matches('/'),
            self.path
        )
    }

    /// Check if series has a specific tag
    pub fn has_tag(&self, tag_id: i64) -> bool {
        self.tags.contains(&tag_id)
    }
}

impl Default for Series {
    fn default() -> Self {
        Self::new(String::new(), 0)
    }
}

/// Series statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesStatistics {
    pub season_count: i32,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub percent_of_episodes: f64,
    pub previous_airing: Option<DateTime<Utc>>,
    pub next_airing: Option<DateTime<Utc>>,
    pub size_on_disk: i64,
}

/// Episode entity - represents a single episode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: i64,
    pub series_id: i64,
    pub tvdb_id: i64,
    pub episode_file_id: Option<i64>,

    // Season/Episode numbers
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_episode_number: Option<i32>,
    pub scene_absolute_episode_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub scene_season_number: Option<i32>,

    // Basic info
    pub title: String,
    pub overview: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub air_date_utc: Option<DateTime<Utc>>,

    // Runtime info
    pub runtime: i32,
    pub finale_type: Option<FinaleType>,

    // Quality
    pub has_file: bool,
    pub monitored: bool,
    pub unverified_scene_numbering: bool,

    // Images
    pub images: Vec<MediaCover>,

    // Timestamps
    pub added: DateTime<Utc>,
    pub last_search_time: Option<DateTime<Utc>>,
}

/// Episode finale types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinaleType {
    Series,
    Season,
    Midseason,
}

impl Episode {
    /// Create a new episode
    pub fn new(series_id: i64, season_number: i32, episode_number: i32) -> Self {
        Self {
            id: 0,
            series_id,
            tvdb_id: 0,
            episode_file_id: None,
            season_number,
            episode_number,
            absolute_episode_number: None,
            scene_absolute_episode_number: None,
            scene_episode_number: None,
            scene_season_number: None,
            title: String::new(),
            overview: None,
            air_date: None,
            air_date_utc: None,
            runtime: 0,
            finale_type: None,
            has_file: false,
            monitored: true,
            unverified_scene_numbering: false,
            images: Vec::new(),
            added: Utc::now(),
            last_search_time: None,
        }
    }

    /// Get the season/episode string (e.g., "S01E05")
    pub fn season_episode_string(&self) -> String {
        format!("S{:02}E{:02}", self.season_number, self.episode_number)
    }

    /// Check if episode has aired
    pub fn has_aired(&self) -> bool {
        match self.air_date_utc {
            Some(air_date) => Utc::now() >= air_date,
            None => false,
        }
    }

    /// Check if episode is missing (monitored but no file)
    pub fn is_missing(&self) -> bool {
        self.monitored && !self.has_file && self.has_aired()
    }
}

/// Episode file entity - represents a downloaded episode file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeFile {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub quality: QualityModel,
    pub languages: Vec<Language>,
    pub media_info: Option<MediaInfoModel>,
    pub original_file_path: Option<String>,
}

/// Quality model with source and resolution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualityModel {
    pub quality: Quality,
    #[serde(default)]
    pub revision: Revision,
}

/// Quality definition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::upper_case_acronyms)]
pub enum Quality {
    #[default]
    Unknown,
    SDTV,
    DVD,
    WEBDL480p,
    HDTV720p,
    WEBDL720p,
    Bluray720p,
    HDTV1080p,
    WEBDL1080p,
    Bluray1080p,
    Bluray1080pRemux,
    WEBDL2160p,
    HDTV2160p,
    Bluray2160p,
    Bluray2160pRemux,
}

/// Quality revision (for proper/repack)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

impl Default for Revision {
    fn default() -> Self {
        Self {
            version: 1,
            real: 0,
            is_repack: false,
        }
    }
}

/// Media info model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfoModel {
    pub audio_bitrate: Option<i64>,
    pub audio_channels: Option<f64>,
    pub audio_codec: Option<String>,
    pub audio_languages: Option<String>,
    pub audio_stream_count: Option<i32>,
    pub video_bit_depth: Option<i32>,
    pub video_bitrate: Option<i64>,
    pub video_codec: Option<String>,
    pub video_fps: Option<f64>,
    pub video_dynamic_range: Option<String>,
    pub video_dynamic_range_type: Option<String>,
    pub resolution: Option<String>,
    pub run_time: Option<String>,
    pub scan_type: Option<String>,
    pub subtitles: Option<String>,
}

/// Add series options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddSeriesOptions {
    pub tvdb_id: i64,
    pub title: String,
    pub quality_profile_id: i64,
    pub language_profile_id: Option<i64>,
    pub root_folder_path: String,
    pub series_type: SeriesType,
    pub monitored: bool,
    pub season_folder: bool,
    pub tags: Vec<i64>,
    pub add_options: AddOptions,
}

/// Additional options when adding a series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddOptions {
    pub ignore_episodes_with_files: bool,
    pub ignore_episodes_without_files: bool,
    pub monitor: NewItemMonitorType,
    pub search_for_missing_episodes: bool,
    pub search_for_cutoff_unmet_episodes: bool,
}

impl Default for AddOptions {
    fn default() -> Self {
        Self {
            ignore_episodes_with_files: false,
            ignore_episodes_without_files: false,
            monitor: NewItemMonitorType::All,
            search_for_missing_episodes: false,
            search_for_cutoff_unmet_episodes: false,
        }
    }
}
