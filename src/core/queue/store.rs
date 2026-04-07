//! Generic JSONL-backed in-memory tracking store.
//!
//! Each content type gets its own `TrackingStore<C>` instance backed by a
//! separate `.jsonl` file.  The store is loaded into memory at startup and
//! served from a `tokio::sync::RwLock<Vec<T>>`.  Mutations acquire a write
//! lock, update the in-memory vec, and atomically flush to disk (write to
//! `.tmp`, then `rename()`).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::{Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::tracked::{AnyTrackedDownload, ContentRef, TrackedDownload};

impl<C: ContentRef> std::fmt::Debug for TrackingStore<C>
where
    for<'a> &'a TrackedDownload<C>: Into<AnyTrackedDownload>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackingStore")
            .field("type", &C::content_type_name())
            .field("path", &self.path)
            .finish()
    }
}

/// In-memory tracking store backed by a JSONL flat file.
pub struct TrackingStore<C: ContentRef> {
    items: RwLock<Vec<TrackedDownload<C>>>,
    next_id: AtomicI64,
    path: PathBuf,
}

impl<C: ContentRef> TrackingStore<C>
where
    for<'a> &'a TrackedDownload<C>: Into<AnyTrackedDownload>,
{
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Load an existing JSONL file, or create an empty store if the file
    /// doesn't exist.
    pub async fn load(path: PathBuf) -> Result<Self> {
        let mut items = Vec::new();
        let mut max_id: i64 = 0;

        if path.exists() {
            let data = tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("Failed to read {}", path.display()))?;

            for (line_num, line) in data.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<TrackedDownload<C>>(line) {
                    Ok(td) => {
                        if td.id > max_id {
                            max_id = td.id;
                        }
                        items.push(td);
                    }
                    Err(e) => {
                        warn!(
                            "Skipping malformed line {} in {}: {}",
                            line_num + 1,
                            path.display(),
                            e
                        );
                    }
                }
            }

            info!(
                "Loaded {} tracked {} downloads from {}",
                items.len(),
                C::content_type_name(),
                path.display()
            );
        } else {
            debug!(
                "No tracking file at {} — starting empty",
                path.display()
            );
        }

        Ok(Self {
            items: RwLock::new(items),
            next_id: AtomicI64::new(max_id + 1),
            path,
        })
    }

    /// Create an empty store at the given path (for migration use).
    pub fn empty(path: PathBuf) -> Self {
        Self {
            items: RwLock::new(Vec::new()),
            next_id: AtomicI64::new(1),
            path,
        }
    }

    // -------------------------------------------------------------------
    // Reads (acquire read lock only)
    // -------------------------------------------------------------------

    /// Return a clone of all tracked items.
    pub async fn get_all(&self) -> Vec<TrackedDownload<C>> {
        self.items.read().await.clone()
    }

    /// Look up a tracked download by its store-assigned ID.
    pub async fn get_by_id(&self, id: i64) -> Option<TrackedDownload<C>> {
        self.items.read().await.iter().find(|td| td.id == id).cloned()
    }

    /// Look up by the download client's natural key.
    pub async fn get_by_download_id(
        &self,
        client_id: i64,
        download_id: &str,
    ) -> Option<TrackedDownload<C>> {
        self.items
            .read()
            .await
            .iter()
            .find(|td| td.client_id == client_id && td.download_id == download_id)
            .cloned()
    }

    /// Collect all `(client_id, download_id)` pairs for suppression checks.
    pub async fn download_ids(&self) -> HashSet<(i64, String)> {
        self.items
            .read()
            .await
            .iter()
            .map(|td| (td.client_id, td.download_id.clone()))
            .collect()
    }

    /// Number of tracked items.
    pub async fn len(&self) -> usize {
        self.items.read().await.len()
    }

    /// Convert all items to type-erased form.
    pub async fn get_all_any(&self) -> Vec<AnyTrackedDownload> {
        self.items.read().await.iter().map(Into::into).collect()
    }

    /// Find by ID and return type-erased form.
    pub async fn find_any(&self, id: i64) -> Option<AnyTrackedDownload> {
        self.items
            .read()
            .await
            .iter()
            .find(|td| td.id == id)
            .map(Into::into)
    }

    // -------------------------------------------------------------------
    // Writes (acquire write lock, mutate, flush)
    // -------------------------------------------------------------------

    /// Insert a new tracked download.  The `id` field on the input is
    /// ignored — a new monotonic ID is assigned.  Returns the assigned ID.
    pub async fn insert(&self, mut item: TrackedDownload<C>) -> Result<i64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        item.id = id;

        let mut items = self.items.write().await;
        items.push(item);
        self.flush(&items).await?;
        Ok(id)
    }

    /// Insert with a specific ID (used during migration from DB).
    /// Updates `next_id` if the provided ID is >= current next.
    pub async fn insert_with_id(&self, item: TrackedDownload<C>) -> Result<()> {
        let id = item.id;
        let mut items = self.items.write().await;
        items.push(item);

        // Ensure next_id stays ahead of all inserted IDs.
        let current_next = self.next_id.load(Ordering::Relaxed);
        if id >= current_next {
            self.next_id.store(id + 1, Ordering::Relaxed);
        }

        self.flush(&items).await
    }

    /// Remove a tracked download by its store-assigned ID.
    /// Returns the removed item, or `None` if not found.
    pub async fn remove(&self, id: i64) -> Option<TrackedDownload<C>> {
        let mut items = self.items.write().await;
        if let Some(pos) = items.iter().position(|td| td.id == id) {
            let removed = items.swap_remove(pos);
            if let Err(e) = self.flush(&items).await {
                warn!("Failed to flush after remove: {}", e);
            }
            Some(removed)
        } else {
            None
        }
    }

    /// Remove a tracked download by the download client's natural key.
    /// Returns the removed item, or `None` if not found.
    pub async fn remove_by_download_id(
        &self,
        client_id: i64,
        download_id: &str,
    ) -> Option<TrackedDownload<C>> {
        let mut items = self.items.write().await;
        if let Some(pos) = items
            .iter()
            .position(|td| td.client_id == client_id && td.download_id == download_id)
        {
            let removed = items.swap_remove(pos);
            if let Err(e) = self.flush(&items).await {
                warn!("Failed to flush after remove: {}", e);
            }
            Some(removed)
        } else {
            None
        }
    }

    /// Mutate a tracked download in place.  The closure receives a mutable
    /// reference; the store flushes after the closure returns.
    pub async fn update<F>(&self, id: i64, f: F) -> Result<bool>
    where
        F: FnOnce(&mut TrackedDownload<C>),
    {
        let mut items = self.items.write().await;
        if let Some(item) = items.iter_mut().find(|td| td.id == id) {
            f(item);
            self.flush(&items).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove all items matching a predicate.  Returns the removed items.
    pub async fn remove_where<F>(&self, predicate: F) -> Vec<TrackedDownload<C>>
    where
        F: Fn(&TrackedDownload<C>) -> bool,
    {
        let mut items = self.items.write().await;
        let mut removed = Vec::new();
        items.retain(|td| {
            if predicate(td) {
                removed.push(td.clone());
                false
            } else {
                true
            }
        });
        if !removed.is_empty() {
            if let Err(e) = self.flush(&items).await {
                warn!("Failed to flush after remove_where: {}", e);
            }
        }
        removed
    }

    // -------------------------------------------------------------------
    // Persistence
    // -------------------------------------------------------------------

    /// Atomically write all items to disk.  Writes to a `.tmp` file first,
    /// then renames over the real file.  `rename()` is atomic on POSIX
    /// filesystems when source and target are on the same mount.
    async fn flush(&self, items: &[TrackedDownload<C>]) -> Result<()> {
        let tmp_path = self.path.with_extension("jsonl.tmp");

        let mut file = tokio::fs::File::create(&tmp_path)
            .await
            .with_context(|| format!("Failed to create {}", tmp_path.display()))?;

        for item in items {
            let line = serde_json::to_string(item)?;
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        file.flush().await?;
        file.sync_all().await?;

        tokio::fs::rename(&tmp_path, &self.path)
            .await
            .with_context(|| {
                format!(
                    "Failed to rename {} -> {}",
                    tmp_path.display(),
                    self.path.display()
                )
            })?;

        Ok(())
    }
}
