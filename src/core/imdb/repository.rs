//! IMDB Repository Stub
//!
//! This module is a stub for the IMDB repository.
//! The actual IMDB data operations are handled by the separate pir9-imdb service.
//! This stub provides the interface for compatibility during the transition.
//!
//! TODO: Remove this file once migration to pir9-imdb service is complete.

use anyhow::Result;

use super::database::{ImdbClient, ImdbDatabase};
use super::models::*;

/// Repository for IMDB data operations (stub - delegates to service)
#[derive(Clone)]
pub struct ImdbRepository {
    client: ImdbClient,
}

impl ImdbRepository {
    pub fn new(_db: ImdbDatabase) -> Self {
        Self {
            client: ImdbClient::from_env(),
        }
    }

    /// Search series by title (delegates to IMDB service)
    pub async fn search_series(&self, query: &str, limit: i32) -> Result<Vec<ImdbSeries>> {
        let results = self.client.search_series(query, limit as u32).await?;
        Ok(results.into_iter().map(|s| ImdbSeries {
            imdb_id: parse_imdb_id(&s.imdb_id).unwrap_or(0),
            title: s.title,
            original_title: s.original_title,
            start_year: s.start_year,
            end_year: s.end_year,
            runtime_minutes: s.runtime_minutes,
            genres: if s.genres.is_empty() { None } else { Some(s.genres.join(",")) },
            is_adult: s.is_adult,
            title_type: "tvSeries".to_string(),
            rating: s.rating.map(|r| r as f32),
            votes: s.votes.map(|v| v as i32),
            last_synced_at: chrono::Utc::now(),
        }).collect())
    }

    /// Get a series by IMDB ID (delegates to IMDB service)
    pub async fn get_series(&self, imdb_id: i64) -> Result<Option<ImdbSeries>> {
        let id_str = super::format_title_id(imdb_id);
        let result = self.client.get_series(&id_str).await?;
        Ok(result.map(|s| ImdbSeries {
            imdb_id,
            title: s.title,
            original_title: s.original_title,
            start_year: s.start_year,
            end_year: s.end_year,
            runtime_minutes: s.runtime_minutes,
            genres: if s.genres.is_empty() { None } else { Some(s.genres.join(",")) },
            is_adult: s.is_adult,
            title_type: "tvSeries".to_string(),
            rating: s.rating.map(|r| r as f32),
            votes: s.votes.map(|v| v as i32),
            last_synced_at: chrono::Utc::now(),
        }))
    }

    /// Get episodes for a series (delegates to IMDB service)
    pub async fn get_episodes(&self, parent_imdb_id: i64) -> Result<Vec<ImdbEpisode>> {
        let id_str = super::format_title_id(parent_imdb_id);
        let results = self.client.get_episodes(&id_str).await?;
        Ok(results.into_iter().map(|e| ImdbEpisode {
            imdb_id: parse_imdb_id(&e.imdb_id).unwrap_or(0),
            parent_imdb_id,
            season_number: e.season_number,
            episode_number: e.episode_number,
            title: e.title,
            runtime_minutes: e.runtime_minutes,
            rating: e.rating.map(|r| r as f32),
            votes: e.votes.map(|v| v as i32),
            air_date: e.air_date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            last_synced_at: chrono::Utc::now(),
        }).collect())
    }

    /// Count series - returns 0 when service not available
    pub async fn count_series(&self) -> Result<i64> {
        if let Some(stats) = self.client.get_stats().await? {
            Ok(stats.series_count)
        } else {
            Ok(0)
        }
    }

    /// Count episodes - returns 0 when service not available
    pub async fn count_episodes(&self) -> Result<i64> {
        if let Some(stats) = self.client.get_stats().await? {
            Ok(stats.episode_count)
        } else {
            Ok(0)
        }
    }

    // ========== Stub methods for sync operations ==========
    // These are no-ops in the main app - sync is handled by pir9-imdb service

    pub async fn upsert_series(&self, _series: &ImdbSeries) -> Result<()> {
        tracing::warn!("IMDB sync operations should be performed by pir9-imdb service");
        Ok(())
    }

    pub async fn upsert_series_batch(&self, _series: &[ImdbSeries]) -> Result<usize> {
        tracing::warn!("IMDB sync operations should be performed by pir9-imdb service");
        Ok(0)
    }

    pub async fn upsert_episode(&self, _episode: &ImdbEpisode) -> Result<()> {
        tracing::warn!("IMDB sync operations should be performed by pir9-imdb service");
        Ok(())
    }

    pub async fn upsert_episodes_batch(&self, _episodes: &[ImdbEpisode]) -> Result<usize> {
        tracing::warn!("IMDB sync operations should be performed by pir9-imdb service");
        Ok(0)
    }

    pub async fn update_series_rating(&self, _imdb_id: i64, _rating: f32, _votes: i32) -> Result<()> {
        Ok(())
    }

    pub async fn update_episode_rating(&self, _imdb_id: i64, _rating: f32, _votes: i32) -> Result<()> {
        Ok(())
    }

    pub async fn start_sync(&self, _dataset_name: &str) -> Result<i64> {
        tracing::warn!("IMDB sync operations should be performed by pir9-imdb service");
        Ok(0)
    }

    pub async fn update_sync_progress(&self, _sync_id: i64, _rows_processed: i64, _rows_inserted: i64, _rows_updated: i64, _last_processed_id: Option<i64>) -> Result<()> {
        Ok(())
    }

    pub async fn complete_sync(&self, _sync_id: i64) -> Result<()> {
        Ok(())
    }

    pub async fn fail_sync(&self, _sync_id: i64, _error: &str) -> Result<()> {
        Ok(())
    }

    pub async fn get_last_sync(&self, _dataset_name: &str) -> Result<Option<ImdbSyncStatus>> {
        Ok(None)
    }

    pub async fn get_current_sync(&self, _dataset_name: &str) -> Result<Option<ImdbSyncStatus>> {
        Ok(None)
    }

    pub async fn get_resumable_sync(&self, _dataset_name: &str) -> Result<Option<(i64, Option<i64>)>> {
        Ok(None)
    }

    pub async fn was_synced_recently(&self, _dataset_name: &str, _days: i64) -> Result<bool> {
        Ok(false)
    }

    pub async fn cleanup_stale_syncs(&self, _max_hours: i64) -> Result<i64> {
        Ok(0)
    }

    pub async fn has_running_sync(&self) -> Result<bool> {
        Ok(false)
    }

    pub async fn cancel_all_running_syncs(&self) -> Result<i64> {
        Ok(0)
    }
}

/// Helper to parse IMDB ID strings
fn parse_imdb_id(id_str: &str) -> Option<i64> {
    super::parse_imdb_id(id_str)
}
