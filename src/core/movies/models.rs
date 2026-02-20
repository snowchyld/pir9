#![allow(dead_code, unused_imports)]
//! Movie domain models
//! Movie and MovieFile entities

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::core::movies::MovieStatusType;
use crate::core::profiles::languages::Language;
use crate::core::profiles::QualityProfile;
use crate::core::tv::{MediaCover, Ratings};

/// Movie entity - represents a movie in the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    pub id: i64,

    // External IDs
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,

    // Basic info
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub status: MovieStatusType,
    pub overview: Option<String>,
    pub monitored: bool,

    // Quality and profile
    pub quality_profile_id: i64,
    pub quality_profile: Option<QualityProfile>,

    // Organization
    pub title_slug: String,
    pub path: String,
    pub root_folder_path: String,

    // Metadata
    pub year: i32,
    pub release_date: Option<NaiveDate>,
    pub physical_release_date: Option<NaiveDate>,
    pub digital_release_date: Option<NaiveDate>,
    pub runtime: i32,
    pub studio: Option<String>,
    pub certification: Option<String>,
    pub genres: Vec<String>,
    pub images: Vec<MediaCover>,
    pub ratings: Option<Ratings>,

    // Tags
    pub tags: HashSet<i64>,

    // File info
    pub has_file: bool,
    pub movie_file_id: Option<i64>,

    // IMDB ratings (from pir9-imdb)
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<i32>,

    // Timestamps
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

impl Movie {
    /// Create a new movie with default values
    pub fn new(title: String, tmdb_id: i64) -> Self {
        let clean_title = Self::clean_title(&title);
        let sort_title = clean_title.clone();
        let title_slug = Self::generate_slug(&title);

        Self {
            id: 0,
            tmdb_id,
            imdb_id: None,
            title,
            clean_title,
            sort_title,
            title_slug,
            status: MovieStatusType::TBA,
            overview: None,
            monitored: true,
            quality_profile_id: 0,
            quality_profile: None,
            path: String::new(),
            root_folder_path: String::new(),
            year: 0,
            release_date: None,
            physical_release_date: None,
            digital_release_date: None,
            runtime: 0,
            studio: None,
            certification: None,
            genres: Vec::new(),
            images: Vec::new(),
            ratings: None,
            tags: HashSet::new(),
            has_file: false,
            movie_file_id: None,
            imdb_rating: None,
            imdb_votes: None,
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

    /// Check if movie needs refresh from metadata source
    pub fn needs_refresh(&self) -> bool {
        match self.last_info_sync {
            None => true,
            Some(last_sync) => {
                let elapsed = Utc::now() - last_sync;
                elapsed.num_hours() > 24
            }
        }
    }

    /// Get the full path for this movie
    pub fn full_path(&self) -> String {
        format!(
            "{}/{}",
            self.root_folder_path.trim_end_matches('/'),
            self.path
        )
    }

    /// Check if movie has a specific tag
    pub fn has_tag(&self, tag_id: i64) -> bool {
        self.tags.contains(&tag_id)
    }
}

impl Default for Movie {
    fn default() -> Self {
        Self::new(String::new(), 0)
    }
}

/// Movie file entity - represents a downloaded movie file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieFile {
    pub id: i64,
    pub movie_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub quality: crate::core::tv::models::QualityModel,
    pub languages: Vec<Language>,
    pub media_info: Option<crate::core::tv::models::MediaInfoModel>,
    pub original_file_path: Option<String>,
    pub edition: Option<String>,
}

/// Options for adding a new movie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMovieOptions {
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub quality_profile_id: i64,
    pub root_folder_path: String,
    pub monitored: bool,
    pub tags: Vec<i64>,
    pub search_for_movie: bool,
}
