//! IMDB Dataset Sync Service (Stub)
//!
//! IMDB sync operations are now handled by the separate pir9-imdb service.
//! This module provides stub implementations for API compatibility.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::database::ImdbDatabase;
use super::models::ImdbSyncStatus;
use super::repository::ImdbRepository;

/// IMDB Dataset Sync Service (stub - delegates to pir9-imdb service)
pub struct ImdbSyncService {
    _repo: ImdbRepository,
}

impl ImdbSyncService {
    pub fn new(db: ImdbDatabase) -> Self {
        Self {
            _repo: ImdbRepository::new(db),
        }
    }

    /// Create a sync service using the default IMDB database path
    pub async fn with_default_db() -> Result<Self> {
        let db = ImdbDatabase::connect(super::DEFAULT_IMDB_DB_PATH).await?;
        Ok(Self::new(db))
    }

    /// Create a sync service optimized for bulk import
    pub async fn with_import_db(path: &str) -> Result<Self> {
        let db = ImdbDatabase::connect_for_import(path).await?;
        Ok(Self::new(db))
    }

    /// Run a full sync of all IMDB datasets
    /// Note: Sync operations should be performed by the pir9-imdb service
    pub async fn sync_all(&self) -> Result<SyncReport> {
        warn!("IMDB sync operations should be performed by the pir9-imdb service");
        info!("To sync IMDB data, use the pir9-imdb service's /api/sync endpoint");

        Ok(SyncReport {
            title_basics: None,
            title_episodes: None,
            title_ratings: None,
            skipped: vec![
                "title.basics.tsv.gz".to_string(),
                "title.episode.tsv.gz".to_string(),
                "title.ratings.tsv.gz".to_string(),
            ],
            errors: vec!["Sync operations delegated to pir9-imdb service".to_string()],
        })
    }

    /// Get sync statistics
    pub async fn get_stats(&self) -> Result<ImdbStats> {
        // Try to get stats from the IMDB service
        use super::database::ImdbClient;
        let client = ImdbClient::from_env();

        if let Some(stats) = client.get_stats().await? {
            Ok(ImdbStats {
                series_count: stats.series_count,
                episode_count: stats.episode_count,
                last_basics_sync: None,
                last_episodes_sync: None,
                last_ratings_sync: None,
            })
        } else {
            Ok(ImdbStats {
                series_count: 0,
                episode_count: 0,
                last_basics_sync: None,
                last_episodes_sync: None,
                last_ratings_sync: None,
            })
        }
    }
}

/// Statistics from a single dataset sync
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStats {
    pub rows_processed: i64,
    pub rows_inserted: i64,
    pub rows_updated: i64,
}

/// Overall sync report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub title_basics: Option<SyncStats>,
    pub title_episodes: Option<SyncStats>,
    pub title_ratings: Option<SyncStats>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

/// IMDB database statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbStats {
    pub series_count: i64,
    pub episode_count: i64,
    pub last_basics_sync: Option<ImdbSyncStatus>,
    pub last_episodes_sync: Option<ImdbSyncStatus>,
    pub last_ratings_sync: Option<ImdbSyncStatus>,
}
