//! RSS feed synchronization

use anyhow::Result;
use chrono::{DateTime, Utc};

use super::clients::create_client_from_model;
use super::search::{apply_minimum_seeders_filter, get_minimum_seeders};
use super::ReleaseInfo;
use crate::core::datastore::models::IndexerDbModel;

/// RSS sync service
pub struct RssSyncService {
    indexers: Vec<IndexerDbModel>,
    last_sync: Option<DateTime<Utc>>,
}

impl RssSyncService {
    pub fn new(indexers: Vec<IndexerDbModel>) -> Self {
        Self {
            indexers,
            last_sync: None,
        }
    }

    /// Perform RSS sync across all enabled indexers
    pub async fn sync(&mut self) -> Result<Vec<ReleaseInfo>> {
        let mut all_releases = Vec::new();

        for indexer in &self.indexers {
            if !indexer.enable_rss {
                continue;
            }

            match self.sync_indexer(indexer).await {
                Ok(releases) => {
                    tracing::info!(
                        "RSS sync from {} returned {} releases",
                        indexer.name,
                        releases.len()
                    );
                    all_releases.extend(releases);
                }
                Err(e) => {
                    tracing::warn!("Indexer {} RSS sync failed: {}", indexer.name, e);
                }
            }
        }

        self.last_sync = Some(Utc::now());

        // Sort by publish date (newest first)
        all_releases.sort_by(|a, b| b.publish_date.cmp(&a.publish_date));

        Ok(all_releases)
    }

    /// Sync a specific indexer's RSS feed
    pub async fn sync_indexer(&self, indexer: &IndexerDbModel) -> Result<Vec<ReleaseInfo>> {
        let client = create_client_from_model(indexer)?;

        // Fetch RSS with a reasonable limit
        let mut releases = client.fetch_rss(Some(100)).await?;

        // Set indexer info on all releases
        for release in &mut releases {
            release.indexer_id = indexer.id;
            release.indexer = indexer.name.clone();
        }

        // Apply minimum seeders filter
        let min_seeders = get_minimum_seeders(indexer);
        let releases = apply_minimum_seeders_filter(releases, min_seeders);

        Ok(releases)
    }

    /// Get last sync time
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.last_sync
    }

    /// Filter releases that are newer than the last sync
    pub fn filter_new_releases(&self, releases: Vec<ReleaseInfo>) -> Vec<ReleaseInfo> {
        if let Some(last_sync) = self.last_sync {
            releases
                .into_iter()
                .filter(|r| r.publish_date > last_sync)
                .collect()
        } else {
            releases
        }
    }
}
