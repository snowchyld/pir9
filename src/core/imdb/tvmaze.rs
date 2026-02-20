//! TVMaze integration for supplemental data (Stub)
//!
//! TVMaze operations are now handled by the separate pir9-imdb service.
//! This module provides stub implementations for API compatibility.
//!
//! API Documentation: https://www.tvmaze.com/api

use anyhow::Result;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;

use super::database::ImdbDatabase;
use super::repository::ImdbRepository;

/// TVMaze supplemental data service (stub - delegates to pir9-imdb service)
pub struct TvMazeService {
    _repo: ImdbRepository,
}

impl TvMazeService {
    pub fn new(db: ImdbDatabase) -> Self {
        Self {
            _repo: ImdbRepository::new(db),
        }
    }

    /// Lookup a show by IMDB ID and fetch episode air dates
    /// Note: TVMaze operations should be performed by the pir9-imdb service
    pub async fn fetch_air_dates_by_imdb(&self, imdb_id: i64) -> Result<usize> {
        warn!(
            "TVMaze operations should be performed by the pir9-imdb service (IMDB ID: {})",
            imdb_id
        );
        Ok(0)
    }

    /// Fetch air dates for all series in our database that are missing them
    /// Note: TVMaze operations should be performed by the pir9-imdb service
    pub async fn backfill_air_dates(&self, _limit: i32) -> Result<BackfillReport> {
        warn!("TVMaze backfill operations should be performed by the pir9-imdb service");
        Ok(BackfillReport::default())
    }

    /// Get upcoming episodes (airing in the next N days)
    /// Note: TVMaze operations should be performed by the pir9-imdb service
    pub async fn get_upcoming_episodes(&self, _days: i32) -> Result<Vec<UpcomingEpisode>> {
        warn!("TVMaze upcoming episode queries should be performed by the pir9-imdb service");
        Ok(vec![])
    }
}

/// TVMaze show lookup response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TvMazeShow {
    pub id: i64,
    pub name: String,
    pub externals: Option<TvMazeExternals>,
}

/// TVMaze external IDs
#[derive(Debug, Clone, Deserialize)]
pub struct TvMazeExternals {
    pub tvdb: Option<i64>,
    pub imdb: Option<String>,
    pub thetvdb: Option<i64>,
}

/// TVMaze episode response
#[derive(Debug, Clone, Deserialize)]
pub struct TvMazeEpisode {
    pub id: i64,
    pub name: Option<String>,
    pub season: Option<i32>,
    pub number: Option<i32>,
    pub airdate: Option<String>,
    #[serde(rename = "airstamp")]
    pub air_stamp: Option<String>,
    pub runtime: Option<i32>,
}

/// Report from air date backfill operation
#[derive(Debug, Clone, Default)]
pub struct BackfillReport {
    pub series_processed: i32,
    pub episodes_updated: i32,
    pub errors: i32,
}

/// Upcoming episode with series info
#[derive(Debug, Clone)]
pub struct UpcomingEpisode {
    pub imdb_id: i64,
    pub parent_imdb_id: i64,
    pub series_title: String,
    pub episode_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub air_date: NaiveDate,
}
