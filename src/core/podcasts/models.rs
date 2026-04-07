#![allow(dead_code, unused_imports)]
//! Podcast domain models
//! Podcast, PodcastEpisode, and PodcastFile entities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::core::tv::MediaCover;

/// Podcast entity - represents a podcast in the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Podcast {
    pub id: i64,

    // Basic info
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub author: Option<String>,

    // Feed
    pub feed_url: String,
    pub website_url: Option<String>,

    // Metadata
    pub genres: Vec<String>,
    pub images: Vec<MediaCover>,

    // Tags
    pub tags: HashSet<i64>,

    // Organization
    pub path: String,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub title_slug: String,

    // Monitoring
    pub monitored: bool,

    // Timestamps
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

impl Podcast {
    /// Create a new podcast with default values
    pub fn new(title: String, feed_url: String) -> Self {
        let clean_title = Self::clean_title(&title);
        let sort_title = clean_title.clone();
        let title_slug = Self::generate_slug(&title);

        Self {
            id: 0,
            title,
            clean_title,
            sort_title,
            overview: None,
            author: None,
            feed_url,
            website_url: None,
            genres: Vec::new(),
            images: Vec::new(),
            tags: HashSet::new(),
            path: String::new(),
            root_folder_path: String::new(),
            quality_profile_id: 1,
            title_slug,
            monitored: true,
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

    /// Check if podcast needs refresh from RSS feed
    pub fn needs_refresh(&self) -> bool {
        match self.last_info_sync {
            None => true,
            Some(last_sync) => {
                let elapsed = Utc::now() - last_sync;
                elapsed.num_hours() > 1
            }
        }
    }

    /// Get the full path for this podcast
    pub fn full_path(&self) -> String {
        format!(
            "{}/{}",
            self.root_folder_path.trim_end_matches('/'),
            self.path
        )
    }

    /// Check if podcast has a specific tag
    pub fn has_tag(&self, tag_id: i64) -> bool {
        self.tags.contains(&tag_id)
    }
}

impl Default for Podcast {
    fn default() -> Self {
        Self::new(String::new(), String::new())
    }
}

/// Podcast episode entity - represents a single podcast episode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastEpisode {
    pub id: i64,
    pub podcast_id: i64,

    // Basic info
    pub title: String,
    pub overview: Option<String>,
    pub episode_number: Option<i32>,
    pub season_number: i32,

    // Timing
    pub air_date_utc: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,

    // Download
    pub download_url: Option<String>,
    pub file_size: Option<i64>,

    // State
    pub has_file: bool,
    pub podcast_file_id: Option<i64>,
    pub monitored: bool,

    // Deduplication
    pub guid: Option<String>,
}

impl PodcastEpisode {
    /// Create a new podcast episode
    pub fn new(podcast_id: i64, title: String) -> Self {
        Self {
            id: 0,
            podcast_id,
            title,
            overview: None,
            episode_number: None,
            season_number: 1,
            air_date_utc: None,
            duration_ms: None,
            download_url: None,
            file_size: None,
            has_file: false,
            podcast_file_id: None,
            monitored: true,
            guid: None,
        }
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

/// Podcast file entity - represents a downloaded podcast file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastFile {
    pub id: i64,
    pub podcast_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub quality: String,
    pub media_info: Option<String>,
    pub date_added: DateTime<Utc>,
}

/// Options for adding a new podcast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPodcastOptions {
    pub feed_url: String,
    pub title: Option<String>,
    pub quality_profile_id: i64,
    pub root_folder_path: String,
    pub monitored: bool,
    pub tags: Vec<i64>,
}
