#![allow(dead_code, unused_imports)]
//! Music domain models
//! Artist, Album, Track, and TrackFile entities

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::core::music::ArtistStatusType;
use crate::core::tv::MediaCover;

/// Artist entity - represents a music artist in the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: i64,

    // External IDs
    pub musicbrainz_id: Option<String>,

    // Basic info
    pub name: String,
    pub clean_name: String,
    pub sort_name: String,
    pub overview: Option<String>,
    pub artist_type: String,
    pub status: ArtistStatusType,
    pub monitored: bool,

    // Quality and profile
    pub quality_profile_id: i64,

    // Organization
    pub title_slug: String,
    pub path: String,
    pub root_folder_path: String,

    // Metadata
    pub genres: Vec<String>,
    pub images: Vec<MediaCover>,

    // Tags
    pub tags: HashSet<i64>,

    // Timestamps
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

impl Artist {
    /// Create a new artist with default values
    pub fn new(name: String) -> Self {
        let clean_name = Self::clean_name(&name);
        let sort_name = clean_name.clone();
        let title_slug = Self::generate_slug(&name);

        Self {
            id: 0,
            musicbrainz_id: None,
            name,
            clean_name,
            sort_name,
            title_slug,
            overview: None,
            artist_type: "group".to_string(),
            status: ArtistStatusType::Continuing,
            monitored: true,
            quality_profile_id: 1,
            path: String::new(),
            root_folder_path: String::new(),
            genres: Vec::new(),
            images: Vec::new(),
            tags: HashSet::new(),
            added: Utc::now(),
            last_info_sync: None,
        }
    }

    /// Clean an artist name for searching/sorting
    fn clean_name(name: &str) -> String {
        name.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate URL-friendly slug
    fn generate_slug(name: &str) -> String {
        name.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
            .replace(' ', "-")
            .replace("--", "-")
            .trim_matches('-')
            .to_string()
    }

    /// Check if artist needs refresh from metadata source
    pub fn needs_refresh(&self) -> bool {
        match self.last_info_sync {
            None => true,
            Some(last_sync) => {
                let elapsed = Utc::now() - last_sync;
                elapsed.num_hours() > 24
            }
        }
    }

    /// Get the full path for this artist
    pub fn full_path(&self) -> String {
        format!(
            "{}/{}",
            self.root_folder_path.trim_end_matches('/'),
            self.path
        )
    }

    /// Check if artist has a specific tag
    pub fn has_tag(&self, tag_id: i64) -> bool {
        self.tags.contains(&tag_id)
    }
}

impl Default for Artist {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Album entity - represents an album within an artist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: i64,
    pub artist_id: i64,
    pub musicbrainz_id: Option<String>,
    pub title: String,
    pub clean_title: String,
    pub album_type: String,
    pub release_date: Option<NaiveDate>,
    pub genres: Vec<String>,
    pub images: Vec<MediaCover>,
    pub monitored: bool,
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

/// Track entity - represents a track within an album
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
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

/// Track file entity - represents a downloaded track file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackFile {
    pub id: i64,
    pub artist_id: i64,
    pub album_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub quality: String,
    pub media_info: Option<String>,
    pub date_added: DateTime<Utc>,
}

/// Options for adding a new artist
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddArtistOptions {
    pub name: String,
    pub musicbrainz_id: Option<String>,
    pub quality_profile_id: i64,
    pub root_folder_path: String,
    pub monitored: bool,
    pub tags: Vec<i64>,
    pub search_for_missing_albums: bool,
}
