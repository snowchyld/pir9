//! TV Shows domain module
//! Contains models and services for Series, Episodes, and Seasons
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod events;
pub mod models;
pub mod repositories;
pub mod services;

pub use models::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Series status types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[repr(i32)]
pub enum SeriesStatusType {
    #[default]
    Continuing = 0,
    Ended = 1,
    Upcoming = 2,
    Deleted = 3,
}

/// Series types (standard, daily, anime)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[repr(i32)]
pub enum SeriesType {
    #[default]
    Standard = 0,
    Daily = 1,
    Anime = 2,
}

/// New item monitoring options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[repr(i32)]
pub enum NewItemMonitorType {
    #[default]
    All = 0,
    Future = 1,
    Missing = 2,
    Existing = 3,
    None = 4,
    Pilot = 5,
    FirstSeason = 6,
    LastSeason = 7,
}

/// Episode monitoring options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringOptions {
    pub ignore_episodes_with_files: bool,
    pub ignore_episodes_without_files: bool,
    pub monitor: NewItemMonitorType,
}

impl Default for MonitoringOptions {
    fn default() -> Self {
        Self {
            ignore_episodes_with_files: false,
            ignore_episodes_without_files: false,
            monitor: NewItemMonitorType::All,
        }
    }
}

/// Actor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub name: String,
    pub character: Option<String>,
    pub order: i32,
    pub image_url: Option<String>,
}

/// Season information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Season {
    pub season_number: i32,
    pub monitored: bool,
    pub statistics: Option<SeasonStatistics>,
}

/// Season statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonStatistics {
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub percent_of_episodes: f64,
    pub previous_airing: Option<DateTime<Utc>>,
    pub next_airing: Option<DateTime<Utc>>,
    pub size_on_disk: i64,
}

/// Ratings information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ratings {
    pub votes: i32,
    pub value: f64,
    pub source: String,
}

/// Media cover image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCover {
    pub cover_type: CoverType,
    pub url: String,
    pub remote_url: Option<String>,
}

/// Cover types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverType {
    Poster,
    Banner,
    Fanart,
    Screenshot,
    Headshot,
    Clearlogo,
}
