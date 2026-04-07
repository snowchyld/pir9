//! Podcasts domain module
//! Contains models and services for Podcasts, PodcastEpisodes, and PodcastFiles
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod events;
pub mod feed;
pub mod models;
pub mod repositories;
pub mod services;

pub use models::*;

use serde::{Deserialize, Serialize};

/// Podcast statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastStatistics {
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub percent_of_episodes: f64,
    pub size_on_disk: i64,
}
