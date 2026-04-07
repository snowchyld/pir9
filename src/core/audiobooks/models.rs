#![allow(dead_code, unused_imports)]
//! Audiobook domain models
//! Audiobook, AudiobookChapter, and AudiobookFile entities

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::core::tv::MediaCover;

/// Audiobook entity - represents an audiobook in the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audiobook {
    pub id: i64,

    // Basic info
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub publisher: Option<String>,

    // External IDs
    pub isbn: Option<String>,
    pub asin: Option<String>,

    // Duration
    pub duration_ms: Option<i64>,
    pub release_date: Option<NaiveDate>,

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
    pub has_file: bool,
    pub audiobook_file_id: Option<i64>,

    // Timestamps
    pub added: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

impl Audiobook {
    /// Create a new audiobook with default values
    pub fn new(title: String) -> Self {
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
            narrator: None,
            publisher: None,
            isbn: None,
            asin: None,
            duration_ms: None,
            release_date: None,
            genres: Vec::new(),
            images: Vec::new(),
            tags: HashSet::new(),
            path: String::new(),
            root_folder_path: String::new(),
            quality_profile_id: 1,
            title_slug,
            monitored: true,
            has_file: false,
            audiobook_file_id: None,
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

    /// Check if audiobook needs refresh from metadata source
    pub fn needs_refresh(&self) -> bool {
        match self.last_info_sync {
            None => true,
            Some(last_sync) => {
                let elapsed = Utc::now() - last_sync;
                elapsed.num_hours() > 24
            }
        }
    }

    /// Get the full path for this audiobook
    pub fn full_path(&self) -> String {
        format!(
            "{}/{}",
            self.root_folder_path.trim_end_matches('/'),
            self.path
        )
    }

    /// Check if audiobook has a specific tag
    pub fn has_tag(&self, tag_id: i64) -> bool {
        self.tags.contains(&tag_id)
    }
}

impl Default for Audiobook {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Audiobook chapter entity - represents a chapter within an audiobook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookChapter {
    pub id: i64,
    pub audiobook_id: i64,
    pub title: String,
    pub chapter_number: i32,
    pub duration_ms: Option<i64>,
    pub has_file: bool,
    pub monitored: bool,
}

impl AudiobookChapter {
    /// Create a new audiobook chapter
    pub fn new(audiobook_id: i64, title: String, chapter_number: i32) -> Self {
        Self {
            id: 0,
            audiobook_id,
            title,
            chapter_number,
            duration_ms: None,
            has_file: false,
            monitored: true,
        }
    }
}

/// Audiobook file entity - represents a downloaded audiobook file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookFile {
    pub id: i64,
    pub audiobook_id: i64,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub quality: String,
    pub media_info: Option<String>,
    pub date_added: DateTime<Utc>,
}

/// Options for adding a new audiobook
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAudiobookOptions {
    pub title: String,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub quality_profile_id: i64,
    pub root_folder_path: String,
    pub monitored: bool,
    pub tags: Vec<i64>,
}
