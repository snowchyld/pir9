//! Audiobooks domain module
//! Contains models and services for Audiobooks, AudiobookChapters, and AudiobookFiles
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod events;
pub mod models;
pub mod repositories;
pub mod services;

pub use models::*;

use serde::{Deserialize, Serialize};

/// Audiobook statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookStatistics {
    pub chapter_count: i32,
    pub chapter_file_count: i32,
    pub percent_of_chapters: f64,
    pub size_on_disk: i64,
}
