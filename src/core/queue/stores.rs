//! Aggregate holder for all per-content-type tracking stores.
//!
//! `TrackedDownloads` is added to `AppState` and provides cross-store
//! operations (find by ID across all types, collect all download IDs, etc.).
//! On first startup it migrates existing records from the `tracked_downloads`
//! DB table into JSONL files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::{info, warn};

use super::store::TrackingStore;
use super::tracked::*;

impl std::fmt::Debug for TrackedDownloads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackedDownloads")
            .field("series", &self.series)
            .field("movies", &self.movies)
            .field("music", &self.music)
            .field("audiobooks", &self.audiobooks)
            .field("podcasts", &self.podcasts)
            .finish()
    }
}

/// All per-content-type tracking stores, plus the suppression list.
///
/// Debug output shows store counts rather than dumping all items.
pub struct TrackedDownloads {
    pub series: TrackingStore<SeriesRef>,
    pub movies: TrackingStore<MovieRef>,
    pub music: TrackingStore<MusicRef>,
    pub audiobooks: TrackingStore<AudiobookRef>,
    pub podcasts: TrackingStore<PodcastRef>,
    pub suppressed: TrackingStore<SuppressedRef>,
}

impl TrackedDownloads {
    /// Create empty stores (for use in tests or fallback contexts where
    /// no data directory is available).
    pub fn empty() -> Self {
        use std::path::PathBuf;
        Self {
            series: TrackingStore::empty(PathBuf::from("/dev/null")),
            movies: TrackingStore::empty(PathBuf::from("/dev/null")),
            music: TrackingStore::empty(PathBuf::from("/dev/null")),
            audiobooks: TrackingStore::empty(PathBuf::from("/dev/null")),
            podcasts: TrackingStore::empty(PathBuf::from("/dev/null")),
            suppressed: TrackingStore::empty(PathBuf::from("/dev/null")),
        }
    }

    /// Load all stores from `{data_dir}/tracked/`.
    /// If the directory doesn't exist but the DB has tracked_downloads,
    /// migrates active records to JSONL (one-time).
    pub async fn load_or_migrate(
        data_dir: &Path,
        db: &crate::core::datastore::Database,
    ) -> Result<Arc<Self>> {
        let tracked_dir = data_dir.join("tracked");

        // Ensure directory exists
        tokio::fs::create_dir_all(&tracked_dir)
            .await
            .with_context(|| format!("Failed to create {}", tracked_dir.display()))?;

        let series_path = tracked_dir.join("series.jsonl");
        let movies_path = tracked_dir.join("movies.jsonl");
        let music_path = tracked_dir.join("music.jsonl");
        let audiobooks_path = tracked_dir.join("audiobooks.jsonl");
        let podcasts_path = tracked_dir.join("podcasts.jsonl");
        let suppressed_path = tracked_dir.join("suppressed.jsonl");

        // Check if we need to migrate from DB
        let needs_migration = !series_path.exists()
            && !movies_path.exists()
            && !music_path.exists()
            && !audiobooks_path.exists()
            && !podcasts_path.exists();

        let stores = Self {
            series: TrackingStore::load(series_path).await?,
            movies: TrackingStore::load(movies_path).await?,
            music: TrackingStore::load(music_path).await?,
            audiobooks: TrackingStore::load(audiobooks_path).await?,
            podcasts: TrackingStore::load(podcasts_path).await?,
            suppressed: TrackingStore::load(suppressed_path).await?,
        };

        if needs_migration {
            if let Err(e) = stores.migrate_from_db(db).await {
                warn!("Failed to migrate tracked downloads from DB: {}", e);
            }
        }

        let total = stores.total_count().await;
        if total > 0 {
            info!(
                "Tracked downloads loaded: {} series, {} movies, {} music, {} audiobooks, {} podcasts ({} suppressed)",
                stores.series.len().await,
                stores.movies.len().await,
                stores.music.len().await,
                stores.audiobooks.len().await,
                stores.podcasts.len().await,
                stores.suppressed.len().await,
            );
        }

        Ok(Arc::new(stores))
    }

    // -------------------------------------------------------------------
    // Cross-store queries
    // -------------------------------------------------------------------

    /// Total number of active tracked downloads across all stores.
    pub async fn total_count(&self) -> usize {
        self.series.len().await
            + self.movies.len().await
            + self.music.len().await
            + self.audiobooks.len().await
            + self.podcasts.len().await
    }

    /// Collect all `(client_id, download_id)` pairs across all stores
    /// (including suppressed) for duplicate/suppression checks.
    pub async fn all_download_ids(&self) -> HashSet<(i64, String)> {
        let mut ids = self.series.download_ids().await;
        ids.extend(self.movies.download_ids().await);
        ids.extend(self.music.download_ids().await);
        ids.extend(self.audiobooks.download_ids().await);
        ids.extend(self.podcasts.download_ids().await);
        ids.extend(self.suppressed.download_ids().await);
        ids
    }

    /// Collect just the `download_id` strings (without client_id) for
    /// backward-compatible checks that only compare download hashes.
    pub async fn all_download_id_strings(&self) -> HashSet<String> {
        self.all_download_ids()
            .await
            .into_iter()
            .map(|(_, did)| did)
            .collect()
    }

    /// Find a tracked download by ID across all stores.
    pub async fn find_by_id(&self, id: i64) -> Option<AnyTrackedDownload> {
        if let Some(any) = self.series.find_any(id).await {
            return Some(any);
        }
        if let Some(any) = self.movies.find_any(id).await {
            return Some(any);
        }
        if let Some(any) = self.music.find_any(id).await {
            return Some(any);
        }
        if let Some(any) = self.audiobooks.find_any(id).await {
            return Some(any);
        }
        if let Some(any) = self.podcasts.find_any(id).await {
            return Some(any);
        }
        None
    }

    /// Remove a tracked download by ID from whichever store contains it.
    /// Returns `true` if found and removed.
    pub async fn remove_by_id(&self, id: i64) -> bool {
        if self.series.remove(id).await.is_some() {
            return true;
        }
        if self.movies.remove(id).await.is_some() {
            return true;
        }
        if self.music.remove(id).await.is_some() {
            return true;
        }
        if self.audiobooks.remove(id).await.is_some() {
            return true;
        }
        if self.podcasts.remove(id).await.is_some() {
            return true;
        }
        false
    }

    /// Remove a tracked download by the download client's natural key,
    /// searching all stores.  Returns `true` if found and removed.
    pub async fn remove_by_download_id(&self, client_id: i64, download_id: &str) -> bool {
        if self
            .series
            .remove_by_download_id(client_id, download_id)
            .await
            .is_some()
        {
            return true;
        }
        if self
            .movies
            .remove_by_download_id(client_id, download_id)
            .await
            .is_some()
        {
            return true;
        }
        if self
            .music
            .remove_by_download_id(client_id, download_id)
            .await
            .is_some()
        {
            return true;
        }
        if self
            .audiobooks
            .remove_by_download_id(client_id, download_id)
            .await
            .is_some()
        {
            return true;
        }
        if self
            .podcasts
            .remove_by_download_id(client_id, download_id)
            .await
            .is_some()
        {
            return true;
        }
        false
    }

    /// Get all tracked downloads as type-erased items (for unified queue view).
    pub async fn get_all_any(&self) -> Vec<AnyTrackedDownload> {
        let mut all = self.series.get_all_any().await;
        all.extend(self.movies.get_all_any().await);
        all.extend(self.music.get_all_any().await);
        all.extend(self.audiobooks.get_all_any().await);
        all.extend(self.podcasts.get_all_any().await);
        all
    }

    /// Check if a `(client_id, download_id)` pair is suppressed.
    pub async fn is_suppressed(&self, client_id: i64, download_id: &str) -> bool {
        self.suppressed
            .get_by_download_id(client_id, download_id)
            .await
            .is_some()
    }

    // -------------------------------------------------------------------
    // Migration from DB
    // -------------------------------------------------------------------

    /// One-time migration: read active tracked downloads from the DB table
    /// and write them into the appropriate JSONL stores.
    async fn migrate_from_db(&self, db: &crate::core::datastore::Database) -> Result<()> {
        let pool = db.pool();

        // Check if the tracked_downloads table exists and has rows
        let count: Option<(i64,)> =
            sqlx::query_as("SELECT COUNT(*) FROM tracked_downloads WHERE status < 4")
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

        let count = match count {
            Some((n,)) if n > 0 => n,
            _ => return Ok(()), // No table or no active rows
        };

        info!(
            "Migrating {} active tracked downloads from DB to JSONL...",
            count
        );

        let rows: Vec<MigrationRow> = sqlx::query_as(
            "SELECT id, download_id, download_client_id, series_id, episode_ids, \
             title, quality, indexer, added, is_upgrade, movie_id, artist_id, \
             audiobook_id, content_type \
             FROM tracked_downloads WHERE status < 4",
        )
        .fetch_all(pool)
        .await?;

        let mut series_count = 0u32;
        let mut movie_count = 0u32;
        let mut music_count = 0u32;
        let mut audiobook_count = 0u32;
        let mut podcast_count = 0u32;

        for row in rows {
            match row.content_type.as_str() {
                "series" | "anime" => {
                    let episode_ids: Vec<i64> =
                        serde_json::from_str(&row.episode_ids).unwrap_or_default();
                    let td = TrackedDownload {
                        id: row.id,
                        download_id: row.download_id,
                        client_id: row.download_client_id,
                        content: SeriesRef {
                            series_id: row.series_id,
                            episode_ids,
                        },
                        title: row.title,
                        quality: row.quality,
                        indexer: row.indexer,
                        added: row.added,
                        is_upgrade: row.is_upgrade,
                    };
                    self.series.insert_with_id(td).await?;
                    series_count += 1;
                }
                "movie" => {
                    if let Some(movie_id) = row.movie_id {
                        let td = TrackedDownload {
                            id: row.id,
                            download_id: row.download_id,
                            client_id: row.download_client_id,
                            content: MovieRef { movie_id },
                            title: row.title,
                            quality: row.quality,
                            indexer: row.indexer,
                            added: row.added,
                            is_upgrade: row.is_upgrade,
                        };
                        self.movies.insert_with_id(td).await?;
                        movie_count += 1;
                    }
                }
                "music" => {
                    if let Some(artist_id) = row.artist_id {
                        let td = TrackedDownload {
                            id: row.id,
                            download_id: row.download_id,
                            client_id: row.download_client_id,
                            content: MusicRef { artist_id },
                            title: row.title,
                            quality: row.quality,
                            indexer: row.indexer,
                            added: row.added,
                            is_upgrade: row.is_upgrade,
                        };
                        self.music.insert_with_id(td).await?;
                        music_count += 1;
                    }
                }
                "audiobook" => {
                    if let Some(audiobook_id) = row.audiobook_id {
                        let td = TrackedDownload {
                            id: row.id,
                            download_id: row.download_id,
                            client_id: row.download_client_id,
                            content: AudiobookRef { audiobook_id },
                            title: row.title,
                            quality: row.quality,
                            indexer: row.indexer,
                            added: row.added,
                            is_upgrade: row.is_upgrade,
                        };
                        self.audiobooks.insert_with_id(td).await?;
                        audiobook_count += 1;
                    }
                }
                "podcast" => {
                    // Podcasts have no FK yet — store with a placeholder
                    let td = TrackedDownload {
                        id: row.id,
                        download_id: row.download_id,
                        client_id: row.download_client_id,
                        content: PodcastRef { podcast_id: 0 },
                        title: row.title,
                        quality: row.quality,
                        indexer: row.indexer,
                        added: row.added,
                        is_upgrade: row.is_upgrade,
                    };
                    self.podcasts.insert_with_id(td).await?;
                    podcast_count += 1;
                }
                other => {
                    warn!(
                        "Unknown content_type '{}' during migration, skipping",
                        other
                    );
                }
            }
        }

        info!(
            "Migration complete: {} series, {} movies, {} music, {} audiobooks, {} podcasts",
            series_count, movie_count, music_count, audiobook_count, podcast_count
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Migration row type — maps to tracked_downloads table columns
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct MigrationRow {
    id: i64,
    download_id: String,
    download_client_id: i64,
    series_id: i64,
    episode_ids: String,
    title: String,
    quality: String,
    indexer: Option<String>,
    added: DateTime<Utc>,
    is_upgrade: bool,
    movie_id: Option<i64>,
    artist_id: Option<i64>,
    audiobook_id: Option<i64>,
    content_type: String,
}
