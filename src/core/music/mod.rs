//! Music domain module
//! Contains models and services for Artists, Albums, Tracks, and TrackFiles
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod events;
pub mod models;
pub mod repositories;
pub mod services;

pub use models::*;

use serde::{Deserialize, Serialize};

/// Artist status types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ArtistStatusType {
    #[default]
    Continuing,
    Ended,
}

impl ArtistStatusType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArtistStatusType::Continuing => "continuing",
            ArtistStatusType::Ended => "ended",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ended" => ArtistStatusType::Ended,
            _ => ArtistStatusType::Continuing,
        }
    }
}

/// Artist statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistStatistics {
    pub album_count: i64,
    pub track_count: i64,
    pub track_file_count: i64,
    pub size_on_disk: i64,
    pub percent_of_tracks: f64,
}
