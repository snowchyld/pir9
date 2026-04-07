//! MusicBrainz Dataset Sync
//!
//! Downloads and parses MusicBrainz JSON dumps from https://data.metabrainz.org/pub/musicbrainz/data/json-dumps/
//!
//! The dumps are tar.xz archives containing one JSON file per entity.
//! - artist.tar.xz: ~2.8M artist JSON files
//! - release-group.tar.xz: ~3.5M release group JSON files
//! - release.tar.xz: ~4.5M release JSON files (huge — selective import)
//!
//! Architecture note: tar::Archive::entries() returns a non-Send iterator (RefCell),
//! so we parse tar entries synchronously in spawn_blocking, sending batches through
//! a channel to the async side for DB upserts.

use std::collections::HashSet;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use futures_util::StreamExt;
use tar::Archive;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use xz2::read::XzDecoder;

use std::sync::Arc;

use crate::db::{DbRepository, ResumeInfo};
use crate::models::*;

/// MusicBrainz JSON dumps base URL (without version directory)
const MB_DUMP_BASE: &str = "https://data.metabrainz.org/pub/musicbrainz/data/json-dumps";

/// URL to fetch the latest dump directory name
const MB_LATEST_URL: &str = "https://data.metabrainz.org/pub/musicbrainz/data/json-dumps/LATEST";

/// Progress log interval (rows processed between DB updates)
const PROGRESS_INTERVAL: i64 = 10_000;

/// Batch size for DB upserts (rows per INSERT statement)
const BATCH_SIZE: usize = 5000;

/// Minimum hours between re-syncing the same dataset
const SKIP_IF_RECENT_HOURS: i64 = 24;

/// Cache directory for downloaded dataset files
const CACHE_DIR: &str = "/data/cache";

thread_local! {
    /// When true, download_dataset skips downloading and uses cached file as-is
    static PROCESS_ONLY_FLAG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Check if a cached file exists and is recent enough to skip re-downloading.
fn cached_file_is_recent(path: &Path, max_age_hours: i64) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            let age = modified.elapsed().unwrap_or(Duration::MAX);
            return age < Duration::from_secs(max_age_hours as u64 * 3600);
        }
    }
    false
}

/// Download a dataset file to a persistent cache directory with progress tracking.
/// If the file already exists and is recent, skips the download.
/// Returns the cache file path on success, or DatasetResult::Cancelled if the token fires.
/// Download a dataset to cache, or use cached file if recent.
/// Reads `process_only` from the shared DownloadProgress state.
async fn download_dataset(
    url: &str,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<std::result::Result<PathBuf, DatasetResult>> {
    let process_only = progress.read().await.process_only;
    download_dataset_inner(url, dataset, token, progress, process_only).await
}

async fn download_dataset_inner(
    url: &str,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    process_only: bool,
) -> Result<std::result::Result<PathBuf, DatasetResult>> {
    // Ensure cache directory exists
    tokio::fs::create_dir_all(CACHE_DIR).await?;

    let cache_path = PathBuf::from(CACHE_DIR).join(dataset);

    // Check if another task is already downloading this dataset
    let lock_path = PathBuf::from(CACHE_DIR).join(format!("{}.downloading", dataset));
    if !process_only && lock_path.exists() {
        info!("{}: already being downloaded, skipping duplicate", dataset);
        return Ok(Ok(cache_path));
    }

    // Process-only mode: use cached file regardless of age, fail if not cached
    if process_only {
        if cache_path.exists() {
            info!("Process-only: using cached {:?}", cache_path);
            {
                let mut p = progress.write().await;
                p.current_file = dataset.to_string();
                p.phase = "parsing".to_string();
                p.percentage = 100.0;
            }
            return Ok(Ok(cache_path));
        }
        anyhow::bail!("{}: not in cache, download first", dataset);
    }

    // Check if cached file exists and is recent enough
    if cached_file_is_recent(&cache_path, SKIP_IF_RECENT_HOURS) {
        info!(
            "Using cached file: {:?} (less than {}h old)",
            cache_path, SKIP_IF_RECENT_HOURS
        );
        {
            let mut p = progress.write().await;
            p.current_file = dataset.to_string();
            p.phase = "parsing".to_string();
            p.percentage = 100.0;
        }
        return Ok(Ok(cache_path));
    }

    // Create lock file to prevent duplicate downloads
    let _ = tokio::fs::write(&lock_path, "").await;

    // Update progress: starting download
    {
        let mut p = progress.write().await;
        p.current_file = dataset.to_string();
        p.phase = "downloading".to_string();
        p.percentage = 0.0;
        p.bytes_done = 0;
        p.total_bytes = 0;
    }

    let client = reqwest::Client::builder()
        .user_agent("pir9-MusicBrainz/0.1.0")
        .timeout(std::time::Duration::from_secs(7200)) // 2h for large dumps
        .build()?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download: {}", response.status());
    }

    let total_size = response.content_length();
    if let Some(total) = total_size {
        let mut p = progress.write().await;
        p.total_bytes = total;
    }

    info!(
        "Downloading {} -> {:?} (size: {})",
        url,
        cache_path,
        total_size.map_or("unknown".to_string(), |s| format!("{} bytes", s))
    );

    // Stream to a temporary file, then rename atomically
    let tmp_path = cache_path.with_extension("tmp");
    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        if token.is_cancelled() {
            info!("Download of {} cancelled", url);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Ok(Err(DatasetResult::Cancelled));
        }

        let chunk = chunk_result?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        // Update progress
        if let Some(total) = total_size {
            let pct = (downloaded as f64 / total as f64) * 100.0;
            let mut p = progress.write().await;
            p.bytes_done = downloaded;
            p.total_bytes = total;
            p.percentage = pct;
        }
    }

    file.flush().await?;
    drop(file);

    // Rename tmp -> final atomically
    tokio::fs::rename(&tmp_path, &cache_path).await?;

    // Remove download lock
    let _ = tokio::fs::remove_file(&lock_path).await;

    info!("Download complete: {} ({} bytes)", dataset, downloaded);

    // Update progress: switching to parsing
    {
        let mut p = progress.write().await;
        p.phase = "parsing".to_string();
        p.percentage = 100.0;
        p.bytes_done = downloaded;
    }

    Ok(Ok(cache_path))
}

/// Result of a dataset sync attempt
pub enum DatasetResult {
    /// Sync completed successfully
    Completed(SyncStats),
    /// Sync was skipped (recently completed)
    Skipped,
    /// Sync was cancelled
    Cancelled,
}

/// Message sent from the blocking tar parser to the async DB writer
enum ParsedBatch<T> {
    /// A batch of parsed entities ready for upsert
    Batch(Vec<T>),
    /// Progress checkpoint: (rows_processed, rows_inserted_so_far, last_id)
    Progress(i64, String),
    /// Parsing is complete
    Done(i64),
    /// Parsing was cancelled at this row count
    Cancelled(i64, String),
}

/// All MusicBrainz dataset filenames, in processing order
/// Small datasets first, then medium, then large
pub const ALL_DATASETS: &[&str] = &[
    "instrument.tar.xz",
    "area.tar.xz",
    "series.tar.xz",
    "event.tar.xz",
    "place.tar.xz",
    "label.tar.xz",
    "artist.tar.xz",
    "work.tar.xz",
    "recording.tar.xz",
    "release-group.tar.xz",
    "release.tar.xz",
];

/// Check whether a dataset name (from the user request) matches a dataset filename.
/// Accepts both exact match ("artist.tar.xz") and short name ("artists", "release-groups", "releases").
fn dataset_matches(filename: &str, requested: &str) -> bool {
    if filename == requested {
        return true;
    }
    // Map short names to filenames
    match requested {
        "artists" | "artist" => filename == "artist.tar.xz",
        "release-groups" | "release_groups" | "release-group" => filename == "release-group.tar.xz",
        "releases" | "release" => filename == "release.tar.xz",
        "labels" | "label" => filename == "label.tar.xz",
        "recordings" | "recording" => filename == "recording.tar.xz",
        "works" | "work" => filename == "work.tar.xz",
        "areas" | "area" => filename == "area.tar.xz",
        "series" => filename == "series.tar.xz",
        "events" | "event" => filename == "event.tar.xz",
        "instruments" | "instrument" => filename == "instrument.tar.xz",
        "places" | "place" => filename == "place.tar.xz",
        _ => filename.starts_with(&format!("{}.", requested))
            || filename.trim_end_matches(".tar.xz") == requested,
    }
}

/// Determine which datasets to operate on, given a user-supplied list.
/// Empty list = all datasets.
fn resolve_datasets(requested: &[String]) -> Vec<&'static str> {
    if requested.is_empty() {
        return ALL_DATASETS.to_vec();
    }
    ALL_DATASETS
        .iter()
        .filter(|ds| requested.iter().any(|r| dataset_matches(ds, r)))
        .copied()
        .collect()
}

/// Format a Duration as a human-readable age string (e.g. "2h 15m", "3d 1h")
fn format_age(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        return format!("{}s", secs);
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{}m", mins);
    }
    let hours = mins / 60;
    let remaining_mins = mins % 60;
    if hours < 24 {
        return format!("{}h {}m", hours, remaining_mins);
    }
    let days = hours / 24;
    let remaining_hours = hours % 24;
    format!("{}d {}h", days, remaining_hours)
}

/// Query dataset file sizes from MusicBrainz servers and local cache without downloading.
/// How often to re-fetch remote sizes (hours)
const REMOTE_SIZE_CACHE_HOURS: i64 = 6;

/// Query dataset file sizes — uses a local JSON cache for remote sizes to avoid
/// hitting upstream servers on every request. Cache refreshes every 6 hours.
pub async fn get_dataset_infos() -> Result<Vec<crate::models::DatasetInfo>> {
    let size_cache_path = PathBuf::from(CACHE_DIR).join(".remote_sizes.json");

    // Load cached remote sizes if fresh enough
    let cached_sizes: std::collections::HashMap<String, u64> =
        if cached_file_is_recent(&size_cache_path, REMOTE_SIZE_CACHE_HOURS) {
            match std::fs::read_to_string(&size_cache_path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
                Err(_) => std::collections::HashMap::new(),
            }
        } else {
            std::collections::HashMap::new()
        };

    // Refresh if cache is empty OR doesn't have all datasets
    let need_refresh = cached_sizes.is_empty()
        || cached_sizes.len() < ALL_DATASETS.len();

    // Only make HEAD requests + resolve LATEST if cache is stale
    let remote_sizes = if need_refresh {
        info!("Remote size cache stale, fetching from upstream...");
        let base_url = resolve_latest_dump_url().await?;
        let client = reqwest::Client::builder()
            .user_agent("pir9-MusicBrainz/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let mut sizes = std::collections::HashMap::new();
        for &dataset in ALL_DATASETS {
            let url = format!("{}/{}", base_url, dataset);
            if let Ok(resp) = client.head(&url).send().await {
                if resp.status().is_success() {
                    if let Some(size) = resp
                        .headers()
                        .get("content-length")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                    {
                        sizes.insert(dataset.to_string(), size);
                    }
                }
            }
        }

        // Persist to cache file
        tokio::fs::create_dir_all(CACHE_DIR).await?;
        if let Ok(json) = serde_json::to_string(&sizes) {
            let _ = tokio::fs::write(&size_cache_path, json).await;
        }

        sizes
    } else {
        cached_sizes
    };

    let mut infos = Vec::with_capacity(ALL_DATASETS.len());

    for &dataset in ALL_DATASETS {
        let remote_size = remote_sizes.get(dataset).copied();

        // Check local cache
        let cache_path = PathBuf::from(CACHE_DIR).join(dataset);
        let (local_size, local_age, cached) = match std::fs::metadata(&cache_path) {
            Ok(meta) => {
                let size = meta.len();
                let age = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.elapsed().ok());
                let age_str = age.map(|a| format_age(a));
                (Some(size), age_str, true)
            }
            Err(_) => (None, None, false),
        };

        infos.push(crate::models::DatasetInfo {
            name: dataset.to_string(),
            remote_size,
            local_size,
            local_age,
            cached,
        });
    }

    Ok(infos)
}

/// Download datasets to cache without processing them.
pub async fn run_download_only(
    requested: &[String],
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<Vec<String>> {
    let datasets = resolve_datasets(requested);
    let base_url = resolve_latest_dump_url().await?;
    let mut downloaded = Vec::new();

    for dataset in datasets {
        if token.is_cancelled() {
            break;
        }
        let url = format!("{}/{}", base_url, dataset);
        match download_dataset(&url, dataset, token, progress).await? {
            Ok(_path) => {
                info!("Downloaded {} to cache", dataset);
                downloaded.push(dataset.to_string());
            }
            Err(DatasetResult::Cancelled) => {
                info!("Download cancelled at {}", dataset);
                break;
            }
            Err(_) => {}
        }
    }

    Ok(downloaded)
}

/// Process already-cached datasets without re-downloading.
/// If a dataset is not in cache, it is skipped with an error.
pub async fn run_process_only(
    db: &DbRepository,
    requested: &[String],
    token: CancellationToken,
    progress: Arc<RwLock<DownloadProgress>>,
) -> Result<SyncReport> {
    let datasets = resolve_datasets(requested);
    let base_url = resolve_latest_dump_url().await?;
    let mut report = SyncReport::default();

    // Set process_only flag on shared state so download_dataset + should_skip read it
    {
        let mut p = progress.write().await;
        p.process_only = true;
    }

    for &dataset in &datasets {
        if token.is_cancelled() {
            break;
        }

        let cache_path = PathBuf::from(CACHE_DIR).join(dataset);
        if !cache_path.exists() {
            report.errors.push(format!("{}: not in cache, download first", dataset));
            continue;
        }

        run_single_dataset_impl(db, dataset, &token, &progress, &base_url, &mut report, true)
            .await;
    }

    // Reset flag
    {
        let mut p = progress.write().await;
        p.process_only = false;
    }

    Ok(report)
}

/// Run a full sync of all MusicBrainz datasets
/// Resolve the latest dump directory from the LATEST file.
/// Returns the full base URL like "https://data.metabrainz.org/pub/musicbrainz/data/json-dumps/20260404-001001"
pub async fn resolve_latest_dump_url() -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    info!("Fetching latest dump version from {}", MB_LATEST_URL);
    let response = client.get(MB_LATEST_URL).send().await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch LATEST: HTTP {}",
            response.status()
        );
    }

    let body = response.text().await?;
    let version = body.trim().to_string();

    if version.is_empty() {
        anyhow::bail!("LATEST file is empty");
    }

    let url = format!("{}/{}", MB_DUMP_BASE, version);
    info!("Latest MusicBrainz dump: {}", url);
    Ok(url)
}

pub async fn run_full_sync(
    db: &DbRepository,
    token: CancellationToken,
    progress: Arc<RwLock<DownloadProgress>>,
) -> Result<SyncReport> {
    run_full_sync_selective(db, &[], token, progress).await
}

/// Run a selective sync of the requested MusicBrainz datasets (empty = all)
pub async fn run_full_sync_selective(
    db: &DbRepository,
    requested: &[String],
    token: CancellationToken,
    progress: Arc<RwLock<DownloadProgress>>,
) -> Result<SyncReport> {
    let datasets = resolve_datasets(requested);
    let mut report = SyncReport::default();

    // Resolve the latest dump URL
    let base_url = resolve_latest_dump_url().await?;

    for &dataset in &datasets {
        if token.is_cancelled() {
            return Ok(report);
        }
        run_single_dataset(db, dataset, &token, &progress, &base_url, &mut report).await;
    }

    Ok(report)
}

/// Run sync for a single named dataset, updating the report
async fn run_single_dataset(
    db: &DbRepository,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
    report: &mut SyncReport,
) {
    run_single_dataset_impl(db, dataset, token, progress, base_url, report, false).await;
}

async fn run_single_dataset_impl(
    db: &DbRepository,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
    report: &mut SyncReport,
    process_only: bool,
) {
    info!(
        "{} {}...",
        if process_only { "Processing" } else { "Syncing" },
        dataset
    );

    let result = match dataset {
        "artist.tar.xz" => sync_artists(db, token, progress, base_url).await,
        "release-group.tar.xz" => sync_release_groups(db, token, progress, base_url).await,
        "release.tar.xz" => sync_releases(db, token, progress, base_url).await,
        "label.tar.xz" => sync_labels(db, token, progress, base_url).await,
        "recording.tar.xz" => sync_recordings(db, token, progress, base_url).await,
        "work.tar.xz" => sync_works(db, token, progress, base_url).await,
        "area.tar.xz" => sync_areas(db, token, progress, base_url).await,
        "series.tar.xz" => sync_series(db, token, progress, base_url).await,
        "event.tar.xz" => sync_events(db, token, progress, base_url).await,
        "instrument.tar.xz" => sync_instruments(db, token, progress, base_url).await,
        "place.tar.xz" => sync_places(db, token, progress, base_url).await,
        _ => {
            report.errors.push(format!("{}: unknown dataset", dataset));
            return;
        }
    };

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            info!(
                "{} completed: {} processed, {} inserted, {} updated",
                dataset, stats.rows_processed, stats.rows_inserted, stats.rows_updated
            );
            match dataset {
                "artist.tar.xz" => report.artists = Some(stats),
                "release-group.tar.xz" => report.release_groups = Some(stats),
                "release.tar.xz" => report.releases = Some(stats),
                "label.tar.xz" => report.labels = Some(stats),
                "recording.tar.xz" => report.recordings = Some(stats),
                "work.tar.xz" => report.works = Some(stats),
                "area.tar.xz" => report.areas = Some(stats),
                "series.tar.xz" => report.series = Some(stats),
                "event.tar.xz" => report.events = Some(stats),
                "instrument.tar.xz" => report.instruments = Some(stats),
                "place.tar.xz" => report.places = Some(stats),
                _ => {}
            }
        }
        Ok(DatasetResult::Skipped) => {
            info!("{} skipped (recently synced)", dataset);
        }
        Ok(DatasetResult::Cancelled) => {
            info!("{} cancelled", dataset);
        }
        Err(e) => {
            error!("{} failed: {}", dataset, e);
            report.errors.push(format!("{}: {}", dataset, e));
        }
    }
}

/// Check if a dataset was completed recently enough to skip
async fn should_skip_dataset(
    db: &DbRepository,
    dataset: &str,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> bool {
    // Process-only mode: never skip — user explicitly requested processing
    if progress.read().await.process_only {
        return false;
    }
    // Never skip if there's an in-progress (incomplete) sync — resume it
    if let Ok(Some(resume)) = db.get_resumable_sync(dataset).await {
        if !resume.last_processed_id.is_empty() {
            info!("{} has an incomplete sync (at id {}), will resume", dataset, resume.last_processed_id);
            return false;
        }
    }
    match db.last_completed_sync_time(dataset).await {
        Ok(Some(completed_at)) => {
            let hours_ago = (Utc::now() - completed_at).num_hours();
            if hours_ago < SKIP_IF_RECENT_HOURS {
                info!(
                    "{} was synced {}h ago (threshold: {}h), skipping",
                    dataset, hours_ago, SKIP_IF_RECENT_HOURS
                );
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Get or create a sync record, handling resume from previous failed/cancelled sync.
async fn get_or_resume_sync(db: &DbRepository, dataset: &str) -> Result<(i64, ResumeInfo)> {
    if let Some(resume) = db.get_resumable_sync(dataset).await? {
        info!(
            "Resuming {} from sync_id={}, last_processed_id={}, prior progress: {} processed",
            dataset, resume.sync_id, resume.last_processed_id, resume.rows_processed
        );
        db.resume_sync(resume.sync_id).await?;
        let sync_id = resume.sync_id;
        return Ok((sync_id, resume));
    }

    let sync_id = db.start_sync(dataset).await?;

    Ok((
        sync_id,
        ResumeInfo {
            sync_id,
            last_processed_id: String::new(),
            rows_processed: 0,
            rows_inserted: 0,
            rows_updated: 0,
        },
    ))
}

// ── Artist sync ─────────────────────────────────────────────────────

async fn sync_artists(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    let dataset = "artist.tar.xz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", base_url, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_artists_inner(db, &url, sync_id, &resume, token, progress).await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn sync_artists_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "artist.tar.xz";
    let resume_from = resume.last_processed_id.clone();
    if !resume_from.is_empty() {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let bytes = std::fs::read(&cache_path)?;
    info!("Read {} bytes from cache, decompressing...", bytes.len());

    let initial_rows = resume.rows_processed;
    let cancel_token = token.clone();

    // Parse tar entries in a blocking task, send batches through channel
    let (tx, mut rx) = mpsc::channel::<ParsedBatch<DbArtist>>(4);

    let parse_handle = tokio::task::spawn_blocking(move || {
        parse_artist_tar(&bytes, &resume_from, initial_rows, &cancel_token, &tx)
    });

    // Receive batches and flush to DB
    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id = resume.last_processed_id.clone();
    let mut cancelled = false;

    while let Some(msg) = rx.recv().await {
        match msg {
            ParsedBatch::Batch(batch) => {
                let count = db.upsert_artist_batch(&batch).await?;
                rows_inserted += count as i64;
            }
            ParsedBatch::Progress(processed, id) => {
                rows_processed = processed;
                last_id = id;
                info!("artist progress: {} rows", rows_processed);
                db.update_sync_progress_with_resume(
                    sync_id,
                    rows_processed,
                    rows_inserted,
                    0,
                    &last_id,
                )
                .await?;
            }
            ParsedBatch::Done(processed) => {
                rows_processed = processed;
            }
            ParsedBatch::Cancelled(processed, id) => {
                rows_processed = processed;
                last_id = id;
                cancelled = true;
            }
        }
    }

    // Wait for the blocking task to finish
    if let Err(e) = parse_handle.await? {
        warn!("Artist tar parse error: {}", e);
    }

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, &last_id)
        .await?;

    if cancelled {
        info!(
            "artist cancelled at row {} (last_id={})",
            rows_processed, last_id
        );
        return Ok(DatasetResult::Cancelled);
    }

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

/// Parse artist tar entries synchronously, sending batches through the channel
#[allow(unused_assignments)]
fn parse_artist_tar(
    bytes: &[u8],
    resume_from: &str,
    initial_rows: i64,
    token: &CancellationToken,
    tx: &mpsc::Sender<ParsedBatch<DbArtist>>,
) -> Result<()> {
    let xz_decoder = XzDecoder::new(bytes);
    let mut archive = Archive::new(xz_decoder);

    // MusicBrainz tar contains: TIMESTAMP, COPYING, README, REPLICATION_SEQUENCE,
    // SCHEMA_SEQUENCE, JSON_DUMPS_SCHEMA_NUMBER, and mbdump/artist (the actual JSONL data)
    // Find the mbdump/* entry and read it line-by-line
    let mut data_entry = None;
    for entry_result in archive.entries()? {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path()?.to_string_lossy().to_string();
        if path.starts_with("mbdump/") {
            data_entry = Some(entry);
            break;
        }
    }

    let data_entry = match data_entry {
        Some(e) => e,
        None => anyhow::bail!("No mbdump/ data entry found in artist tar"),
    };

    let reader = std::io::BufReader::new(data_entry);

    let mut rows_processed = initial_rows;
    let mut last_id = resume_from.to_string();
    let mut batch: Vec<DbArtist> = Vec::with_capacity(BATCH_SIZE);

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.is_empty() {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mbid = match json.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        // Skip rows we've already processed (resume support)
        if !resume_from.is_empty() && mbid.as_str() <= resume_from {
            continue;
        }

        batch.push(parse_artist_json(&json, mbid.clone()));

        rows_processed += 1;
        last_id = mbid;

        if batch.len() >= BATCH_SIZE {
            let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
            if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                return Ok(()); // Receiver dropped
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 {
            // Send remaining batch before progress checkpoint
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                    return Ok(());
                }
            }

            if tx
                .blocking_send(ParsedBatch::Progress(rows_processed, last_id.clone()))
                .is_err()
            {
                return Ok(());
            }
        }

        // Cooperative cancellation check
        if rows_processed % PROGRESS_INTERVAL == 0 && token.is_cancelled() {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                let _ = tx.blocking_send(ParsedBatch::Batch(send_batch));
            }
            let _ = tx.blocking_send(ParsedBatch::Cancelled(rows_processed, last_id));
            return Ok(());
        }
    }

    // Send remaining batch
    if !batch.is_empty() {
        let _ = tx.blocking_send(ParsedBatch::Batch(batch));
    }
    let _ = tx.blocking_send(ParsedBatch::Done(rows_processed));

    Ok(())
}

fn parse_artist_json(json: &serde_json::Value, mbid: String) -> DbArtist {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let sort_name = json
        .get("sort-name")
        .and_then(|v| v.as_str())
        .unwrap_or(&name)
        .to_string();

    let artist_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);
    let gender = json
        .get("gender")
        .and_then(|v| v.as_str())
        .map(String::from);
    let area = json
        .get("area")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let life_span = json.get("life-span");
    let begin_date = life_span
        .and_then(|ls| ls.get("begin"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let end_date = life_span
        .and_then(|ls| ls.get("end"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let genres = extract_genre_names(json);
    let tags = extract_tag_names(json);
    let (rating, rating_count) = extract_rating(json);

    DbArtist {
        mbid,
        name,
        sort_name,
        artist_type,
        gender,
        area,
        begin_date,
        end_date,
        disambiguation,
        genres,
        tags,
        rating,
        rating_count,
        last_synced_at: Utc::now(),
    }
}

// ── Release Group sync ──────────────────────────────────────────────

async fn sync_release_groups(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    let dataset = "release-group.tar.xz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", base_url, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_release_groups_inner(db, &url, sync_id, &resume, token, progress).await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn sync_release_groups_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "release-group.tar.xz";
    let resume_from = resume.last_processed_id.clone();
    if !resume_from.is_empty() {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let bytes = std::fs::read(&cache_path)?;
    info!("Read {} bytes from cache, decompressing...", bytes.len());

    let initial_rows = resume.rows_processed;
    let cancel_token = token.clone();

    let (tx, mut rx) = mpsc::channel::<ParsedBatch<DbReleaseGroup>>(4);

    let parse_handle = tokio::task::spawn_blocking(move || {
        parse_release_group_tar(&bytes, &resume_from, initial_rows, &cancel_token, &tx)
    });

    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id = resume.last_processed_id.clone();
    let mut cancelled = false;

    while let Some(msg) = rx.recv().await {
        match msg {
            ParsedBatch::Batch(batch) => {
                let count = db.upsert_release_group_batch(&batch).await?;
                rows_inserted += count as i64;
            }
            ParsedBatch::Progress(processed, id) => {
                rows_processed = processed;
                last_id = id;
                info!("release-group progress: {} rows", rows_processed);
                db.update_sync_progress_with_resume(
                    sync_id,
                    rows_processed,
                    rows_inserted,
                    0,
                    &last_id,
                )
                .await?;
            }
            ParsedBatch::Done(processed) => {
                rows_processed = processed;
            }
            ParsedBatch::Cancelled(processed, id) => {
                rows_processed = processed;
                last_id = id;
                cancelled = true;
            }
        }
    }

    if let Err(e) = parse_handle.await? {
        warn!("Release group tar parse error: {}", e);
    }

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, &last_id)
        .await?;

    if cancelled {
        info!(
            "release-group cancelled at row {} (last_id={})",
            rows_processed, last_id
        );
        return Ok(DatasetResult::Cancelled);
    }

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

#[allow(unused_assignments)]
fn parse_release_group_tar(
    bytes: &[u8],
    resume_from: &str,
    initial_rows: i64,
    token: &CancellationToken,
    tx: &mpsc::Sender<ParsedBatch<DbReleaseGroup>>,
) -> Result<()> {
    let xz_decoder = XzDecoder::new(bytes);
    let mut archive = Archive::new(xz_decoder);

    // Find the mbdump/* JSONL data entry
    let mut data_entry = None;
    for entry_result in archive.entries()? {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path()?.to_string_lossy().to_string();
        if path.starts_with("mbdump/") {
            data_entry = Some(entry);
            break;
        }
    }

    let data_entry = match data_entry {
        Some(e) => e,
        None => anyhow::bail!("No mbdump/ data entry found in release-group tar"),
    };

    let reader = std::io::BufReader::new(data_entry);

    let mut rows_processed = initial_rows;
    let mut last_id = resume_from.to_string();
    let mut batch: Vec<DbReleaseGroup> = Vec::with_capacity(BATCH_SIZE);

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.is_empty() {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mbid = match json.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        if !resume_from.is_empty() && mbid.as_str() <= resume_from {
            continue;
        }

        batch.push(parse_release_group_json(&json, mbid.clone()));

        rows_processed += 1;
        last_id = mbid;

        if batch.len() >= BATCH_SIZE {
            let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
            if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                return Ok(());
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                    return Ok(());
                }
            }
            if tx
                .blocking_send(ParsedBatch::Progress(rows_processed, last_id.clone()))
                .is_err()
            {
                return Ok(());
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 && token.is_cancelled() {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                let _ = tx.blocking_send(ParsedBatch::Batch(send_batch));
            }
            let _ = tx.blocking_send(ParsedBatch::Cancelled(rows_processed, last_id));
            return Ok(());
        }
    }

    if !batch.is_empty() {
        let _ = tx.blocking_send(ParsedBatch::Batch(batch));
    }
    let _ = tx.blocking_send(ParsedBatch::Done(rows_processed));

    Ok(())
}

fn parse_release_group_json(json: &serde_json::Value, mbid: String) -> DbReleaseGroup {
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (artist_mbid, artist_name) = extract_artist_credit(json);

    let primary_type = json
        .get("primary-type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let secondary_types = json
        .get("secondary-types")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let types: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            serde_json::to_string(&types).unwrap_or_else(|_| "[]".to_string())
        })
        .unwrap_or_else(|| "[]".to_string());

    let first_release_date = json
        .get("first-release-date")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let genres = extract_genre_names(json);
    let tags = extract_tag_names(json);
    let (rating, rating_count) = extract_rating(json);

    DbReleaseGroup {
        mbid,
        title,
        artist_mbid,
        artist_name,
        primary_type,
        secondary_types,
        first_release_date,
        genres,
        tags,
        rating,
        rating_count,
        last_synced_at: Utc::now(),
    }
}

// ── Release sync ────────────────────────────────────────────────────

async fn sync_releases(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    let dataset = "release.tar.xz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", base_url, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_releases_inner(db, &url, sync_id, &resume, token, progress).await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn sync_releases_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "release.tar.xz";
    let resume_from = resume.last_processed_id.clone();
    if !resume_from.is_empty() {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    // Pre-load known release group MBIDs for filtering
    let known_release_groups: HashSet<String> =
        sqlx::query_scalar("SELECT mbid FROM mb_release_groups")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    info!(
        "Filtering releases for {} known release groups",
        known_release_groups.len()
    );

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let bytes = std::fs::read(&cache_path)?;
    info!("Read {} bytes from cache, decompressing...", bytes.len());

    let initial_rows = resume.rows_processed;
    let cancel_token = token.clone();

    let (tx, mut rx) = mpsc::channel::<ParsedBatch<ParsedRelease>>(4);

    let parse_handle = tokio::task::spawn_blocking(move || {
        parse_release_tar(
            &bytes,
            &resume_from,
            initial_rows,
            &known_release_groups,
            &cancel_token,
            &tx,
        )
    });

    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id = resume.last_processed_id.clone();
    let mut cancelled = false;

    while let Some(msg) = rx.recv().await {
        match msg {
            ParsedBatch::Batch(batch) => {
                // Split ParsedRelease into releases + tracks
                let releases: Vec<DbRelease> = batch.iter().map(|p| p.release.clone()).collect();
                let count = db.upsert_release_batch(&releases).await?;
                rows_inserted += count as i64;

                // Collect all tracks from all releases and bulk-insert
                let all_tracks: Vec<DbReleaseTrack> = batch
                    .iter()
                    .flat_map(|p| p.tracks.iter().cloned())
                    .collect();
                if !all_tracks.is_empty() {
                    if let Err(e) = db.upsert_release_tracks_bulk(&all_tracks).await {
                        tracing::warn!("Failed to upsert {} release tracks: {}", all_tracks.len(), e);
                    }
                }
            }
            ParsedBatch::Progress(processed, id) => {
                rows_processed = processed;
                last_id = id;
                info!("release progress: {} rows", rows_processed);
                db.update_sync_progress_with_resume(
                    sync_id,
                    rows_processed,
                    rows_inserted,
                    0,
                    &last_id,
                )
                .await?;
            }
            ParsedBatch::Done(processed) => {
                rows_processed = processed;
            }
            ParsedBatch::Cancelled(processed, id) => {
                rows_processed = processed;
                last_id = id;
                cancelled = true;
            }
        }
    }

    if let Err(e) = parse_handle.await? {
        warn!("Release tar parse error: {}", e);
    }

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, &last_id)
        .await?;

    if cancelled {
        info!(
            "release cancelled at row {} (last_id={})",
            rows_processed, last_id
        );
        return Ok(DatasetResult::Cancelled);
    }

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

#[allow(unused_assignments)]
fn parse_release_tar(
    bytes: &[u8],
    resume_from: &str,
    initial_rows: i64,
    known_release_groups: &HashSet<String>,
    token: &CancellationToken,
    tx: &mpsc::Sender<ParsedBatch<ParsedRelease>>,
) -> Result<()> {
    let xz_decoder = XzDecoder::new(bytes);
    let mut archive = Archive::new(xz_decoder);

    // Find the mbdump/* JSONL data entry
    let mut data_entry = None;
    for entry_result in archive.entries()? {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path()?.to_string_lossy().to_string();
        if path.starts_with("mbdump/") {
            data_entry = Some(entry);
            break;
        }
    }

    let data_entry = match data_entry {
        Some(e) => e,
        None => anyhow::bail!("No mbdump/ data entry found in release tar"),
    };

    let reader = std::io::BufReader::new(data_entry);

    let mut rows_processed = initial_rows;
    let mut last_id = resume_from.to_string();
    let mut batch: Vec<ParsedRelease> = Vec::with_capacity(BATCH_SIZE);

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.is_empty() {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mbid = match json.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        if !resume_from.is_empty() && mbid.as_str() <= resume_from {
            continue;
        }

        // Get release group MBID — only import releases for groups we have
        let release_group_mbid = match json
            .get("release-group")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
        {
            Some(rg_id) => {
                if !known_release_groups.contains(rg_id) {
                    continue;
                }
                rg_id.to_string()
            }
            None => continue,
        };

        batch.push(parse_release_json(&json, mbid.clone(), release_group_mbid));

        rows_processed += 1;
        last_id = mbid;

        if batch.len() >= BATCH_SIZE {
            let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
            if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                return Ok(());
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                    return Ok(());
                }
            }
            if tx
                .blocking_send(ParsedBatch::Progress(rows_processed, last_id.clone()))
                .is_err()
            {
                return Ok(());
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 && token.is_cancelled() {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                let _ = tx.blocking_send(ParsedBatch::Batch(send_batch));
            }
            let _ = tx.blocking_send(ParsedBatch::Cancelled(rows_processed, last_id));
            return Ok(());
        }
    }

    if !batch.is_empty() {
        let _ = tx.blocking_send(ParsedBatch::Batch(batch));
    }
    let _ = tx.blocking_send(ParsedBatch::Done(rows_processed));

    Ok(())
}

/// Parsed release with its track listing
pub struct ParsedRelease {
    pub release: DbRelease,
    pub tracks: Vec<DbReleaseTrack>,
}

fn parse_release_json(
    json: &serde_json::Value,
    mbid: String,
    release_group_mbid: String,
) -> ParsedRelease {
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (artist_mbid_str, artist_name_str) = extract_artist_credit(json);
    let artist_mbid = if artist_mbid_str.is_empty() {
        None
    } else {
        Some(artist_mbid_str)
    };
    let artist_name = if artist_name_str.is_empty() {
        None
    } else {
        Some(artist_name_str)
    };

    let date = json
        .get("date")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let country = json
        .get("country")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let barcode = json
        .get("barcode")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let packaging = json
        .get("packaging")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    // Parse media/tracks
    let media_arr = json.get("media").and_then(|v| v.as_array());

    let track_count: i32 = media_arr
        .map(|media| {
            media
                .iter()
                .filter_map(|m| m.get("track-count").and_then(|v| v.as_i64()))
                .sum::<i64>() as i32
        })
        .unwrap_or(0);

    // Extract individual tracks from media[].tracks[]
    let mut tracks = Vec::new();
    if let Some(media) = media_arr {
        for (disc_idx, medium) in media.iter().enumerate() {
            let disc_number = medium
                .get("position")
                .and_then(|v| v.as_i64())
                .unwrap_or(disc_idx as i64 + 1) as i32;

            if let Some(track_arr) = medium.get("tracks").and_then(|v| v.as_array()) {
                for track in track_arr {
                    let position = track
                        .get("position")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    let track_title = track
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let length_ms = track
                        .get("length")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    let recording_mbid = track
                        .get("recording")
                        .and_then(|r| r.get("id"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    tracks.push(DbReleaseTrack {
                        release_mbid: mbid.clone(),
                        disc_number,
                        position,
                        title: track_title,
                        recording_mbid,
                        length_ms,
                    });
                }
            }
        }
    }

    ParsedRelease {
        release: DbRelease {
            mbid,
            release_group_mbid,
            title,
            artist_mbid,
            artist_name,
            date,
            country,
            status,
            barcode,
            packaging,
            track_count,
            last_synced_at: Utc::now(),
        },
        tracks,
    }
}

// ── JSON extraction helpers ─────────────────────────────────────────

/// Extract genre names from a MusicBrainz JSON entity as a JSON array string
fn extract_genre_names(json: &serde_json::Value) -> String {
    let genres: Vec<String> = json
        .get("genres")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&genres).unwrap_or_else(|_| "[]".to_string())
}

/// Extract tag names from a MusicBrainz JSON entity as a JSON array string
fn extract_tag_names(json: &serde_json::Value) -> String {
    let tags: Vec<String> = json
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string())
}

/// Extract rating value and vote count from a MusicBrainz JSON entity
fn extract_rating(json: &serde_json::Value) -> (Option<f64>, Option<i32>) {
    let rating_obj = json.get("rating");
    let value = rating_obj
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_f64());
    let count = rating_obj
        .and_then(|r| r.get("votes-count"))
        .and_then(|v| v.as_i64())
        .map(|c| c as i32);

    (value, count)
}

/// Extract primary artist MBID and name from artist-credit array
fn extract_artist_credit(json: &serde_json::Value) -> (String, String) {
    let credit = json
        .get("artist-credit")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first());

    let artist_mbid = credit
        .and_then(|c| c.get("artist"))
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist_name = credit
        .and_then(|c| c.get("artist"))
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    (artist_mbid, artist_name)
}

/// Extract a JSON array of strings from a field (e.g., ISRCs, ISWCs, languages)
fn extract_string_array(json: &serde_json::Value, field: &str) -> String {
    let items: Vec<String> = json
        .get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    // Some arrays have string elements directly, others have objects with "name"
                    v.as_str()
                        .map(String::from)
                        .or_else(|| v.get("name").and_then(|n| n.as_str()).map(String::from))
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
}

// ============================================================================
// Generic tar parse helper — reduces boilerplate for simple datasets
// ============================================================================

/// Generic JSONL-in-tar parser. Finds the `mbdump/*` entry, reads line by line,
/// parses each JSON line with the provided closure, and sends batches through the channel.
#[allow(unused_assignments)]
fn parse_generic_tar<T: Send + 'static>(
    bytes: &[u8],
    resume_from: &str,
    initial_rows: i64,
    token: &CancellationToken,
    tx: &mpsc::Sender<ParsedBatch<T>>,
    dataset_name: &str,
    parse_fn: impl Fn(&serde_json::Value, String) -> T,
) -> Result<()> {
    let xz_decoder = XzDecoder::new(bytes);
    let mut archive = Archive::new(xz_decoder);

    let mut data_entry = None;
    for entry_result in archive.entries()? {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path()?.to_string_lossy().to_string();
        if path.starts_with("mbdump/") {
            data_entry = Some(entry);
            break;
        }
    }

    let data_entry = match data_entry {
        Some(e) => e,
        None => anyhow::bail!("No mbdump/ data entry found in {} tar", dataset_name),
    };

    let reader = std::io::BufReader::new(data_entry);

    let mut rows_processed = initial_rows;
    let mut last_id = resume_from.to_string();
    let mut batch: Vec<T> = Vec::with_capacity(BATCH_SIZE);

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.is_empty() {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mbid = match json.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        if !resume_from.is_empty() && mbid.as_str() <= resume_from {
            continue;
        }

        batch.push(parse_fn(&json, mbid.clone()));

        rows_processed += 1;
        last_id = mbid;

        if batch.len() >= BATCH_SIZE {
            let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
            if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                return Ok(());
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                if tx.blocking_send(ParsedBatch::Batch(send_batch)).is_err() {
                    return Ok(());
                }
            }
            if tx
                .blocking_send(ParsedBatch::Progress(rows_processed, last_id.clone()))
                .is_err()
            {
                return Ok(());
            }
        }

        if rows_processed % PROGRESS_INTERVAL == 0 && token.is_cancelled() {
            if !batch.is_empty() {
                let send_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                let _ = tx.blocking_send(ParsedBatch::Batch(send_batch));
            }
            let _ = tx.blocking_send(ParsedBatch::Cancelled(rows_processed, last_id));
            return Ok(());
        }
    }

    if !batch.is_empty() {
        let _ = tx.blocking_send(ParsedBatch::Batch(batch));
    }
    let _ = tx.blocking_send(ParsedBatch::Done(rows_processed));

    Ok(())
}

/// Generic sync wrapper — handles download, channel setup, DB upsert loop, and progress tracking.
/// `upsert_fn` receives a batch and returns rows affected.
async fn sync_generic<T: Send + 'static>(
    db: &DbRepository,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
    parse_fn: impl Fn(&serde_json::Value, String) -> T + Send + 'static,
    upsert_fn: impl AsyncUpsertFn<T>,
) -> Result<DatasetResult> {
    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", base_url, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_generic_inner(
        db, &url, dataset, sync_id, &resume, token, progress, parse_fn, &upsert_fn,
    )
    .await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

/// Trait for async batch upsert functions
trait AsyncUpsertFn<T>: Send + Sync {
    fn upsert(
        &self,
        db: &DbRepository,
        batch: &[T],
    ) -> impl std::future::Future<Output = Result<u64>> + Send;
}

/// Macro to generate upsert structs for each entity type
macro_rules! upsert_struct {
    ($name:ident, $db_type:ty, $method:ident) => {
        struct $name;

        impl AsyncUpsertFn<$db_type> for $name {
            async fn upsert(&self, db: &DbRepository, batch: &[$db_type]) -> Result<u64> {
                db.$method(batch).await
            }
        }
    };
}

upsert_struct!(UpsertLabel, DbLabel, upsert_label_batch);
upsert_struct!(UpsertRecording, DbRecording, upsert_recording_batch);
upsert_struct!(UpsertWork, DbWork, upsert_work_batch);
upsert_struct!(UpsertArea, DbArea, upsert_area_batch);
upsert_struct!(UpsertSeries, DbSeries, upsert_series_batch);
upsert_struct!(UpsertEvent, DbEvent, upsert_event_batch);
upsert_struct!(UpsertInstrument, DbInstrument, upsert_instrument_batch);
upsert_struct!(UpsertPlace, DbPlace, upsert_place_batch);

async fn sync_generic_inner<T: Send + 'static>(
    db: &DbRepository,
    url: &str,
    dataset: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    parse_fn: impl Fn(&serde_json::Value, String) -> T + Send + 'static,
    upsert_fn: &impl AsyncUpsertFn<T>,
) -> Result<DatasetResult> {
    let resume_from = resume.last_processed_id.clone();
    if !resume_from.is_empty() {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let bytes = std::fs::read(&cache_path)?;
    info!("Read {} bytes from cache, decompressing...", bytes.len());

    let initial_rows = resume.rows_processed;
    let cancel_token = token.clone();
    let dataset_name = dataset.to_string();

    let (tx, mut rx) = mpsc::channel::<ParsedBatch<T>>(4);

    let parse_handle = tokio::task::spawn_blocking(move || {
        parse_generic_tar(
            &bytes,
            &resume_from,
            initial_rows,
            &cancel_token,
            &tx,
            &dataset_name,
            parse_fn,
        )
    });

    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id = resume.last_processed_id.clone();
    let mut cancelled = false;

    while let Some(msg) = rx.recv().await {
        match msg {
            ParsedBatch::Batch(batch) => {
                let count = upsert_fn.upsert(db, &batch).await?;
                rows_inserted += count as i64;
            }
            ParsedBatch::Progress(processed, id) => {
                rows_processed = processed;
                last_id = id;
                info!("{} progress: {} rows", dataset, rows_processed);
                db.update_sync_progress_with_resume(
                    sync_id,
                    rows_processed,
                    rows_inserted,
                    0,
                    &last_id,
                )
                .await?;
            }
            ParsedBatch::Done(processed) => {
                rows_processed = processed;
            }
            ParsedBatch::Cancelled(processed, id) => {
                rows_processed = processed;
                last_id = id;
                cancelled = true;
            }
        }
    }

    if let Err(e) = parse_handle.await? {
        warn!("{} tar parse error: {}", dataset, e);
    }

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, &last_id)
        .await?;

    if cancelled {
        info!(
            "{} cancelled at row {} (last_id={})",
            dataset, rows_processed, last_id
        );
        return Ok(DatasetResult::Cancelled);
    }

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

// ── Label sync ────────────────────────────────────────────────────

async fn sync_labels(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "label.tar.xz",
        token,
        progress,
        base_url,
        parse_label_json,
        UpsertLabel,
    )
    .await
}

fn parse_label_json(json: &serde_json::Value, mbid: String) -> DbLabel {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sort_name = json
        .get("sort-name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let label_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let area = json
        .get("area")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let label_code = json
        .get("label-code")
        .and_then(|v| v.as_i64())
        .map(|c| c as i32);

    let life_span = json.get("life-span");
    let begin_date = life_span
        .and_then(|ls| ls.get("begin"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let end_date = life_span
        .and_then(|ls| ls.get("end"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let genres = extract_genre_names(json);
    let tags = extract_tag_names(json);

    DbLabel {
        mbid,
        name,
        sort_name,
        label_type,
        area,
        label_code,
        begin_date,
        end_date,
        disambiguation,
        genres,
        tags,
        last_synced_at: Utc::now(),
    }
}

// ── Recording sync ────────────────────────────────────────────────

async fn sync_recordings(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "recording.tar.xz",
        token,
        progress,
        base_url,
        parse_recording_json,
        UpsertRecording,
    )
    .await
}

fn parse_recording_json(json: &serde_json::Value, mbid: String) -> DbRecording {
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let credit = json
        .get("artist-credit")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first());

    let artist_mbid = credit
        .and_then(|c| c.get("artist"))
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let artist_name = credit
        .and_then(|c| c.get("artist"))
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let length_ms = json
        .get("length")
        .and_then(|v| v.as_i64())
        .map(|l| l as i32);

    let first_release_date = json
        .get("first-release-date")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let isrcs = extract_string_array(json, "isrcs");
    let genres = extract_genre_names(json);
    let tags = extract_tag_names(json);

    DbRecording {
        mbid,
        title,
        artist_mbid,
        artist_name,
        length_ms,
        first_release_date,
        isrcs,
        genres,
        tags,
        last_synced_at: Utc::now(),
    }
}

// ── Work sync ─────────────────────────────────────────────────────

async fn sync_works(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "work.tar.xz",
        token,
        progress,
        base_url,
        parse_work_json,
        UpsertWork,
    )
    .await
}

fn parse_work_json(json: &serde_json::Value, mbid: String) -> DbWork {
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let work_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let languages = extract_string_array(json, "languages");
    let iswcs = extract_string_array(json, "iswcs");

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let genres = extract_genre_names(json);
    let tags = extract_tag_names(json);

    DbWork {
        mbid,
        title,
        work_type,
        languages,
        iswcs,
        disambiguation,
        genres,
        tags,
        last_synced_at: Utc::now(),
    }
}

// ── Area sync ─────────────────────────────────────────────────────

async fn sync_areas(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "area.tar.xz",
        token,
        progress,
        base_url,
        parse_area_json,
        UpsertArea,
    )
    .await
}

fn parse_area_json(json: &serde_json::Value, mbid: String) -> DbArea {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sort_name = json
        .get("sort-name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let area_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let iso_3166_1 = json
        .get("iso-3166-1-codes")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(String::from);

    let iso_3166_2 = json
        .get("iso-3166-2-codes")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(String::from);

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    DbArea {
        mbid,
        name,
        sort_name,
        area_type,
        iso_3166_1,
        iso_3166_2,
        disambiguation,
        last_synced_at: Utc::now(),
    }
}

// ── Series sync ───────────────────────────────────────────────────

async fn sync_series(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "series.tar.xz",
        token,
        progress,
        base_url,
        parse_series_json,
        UpsertSeries,
    )
    .await
}

fn parse_series_json(json: &serde_json::Value, mbid: String) -> DbSeries {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let series_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    DbSeries {
        mbid,
        name,
        series_type,
        disambiguation,
        last_synced_at: Utc::now(),
    }
}

// ── Event sync ────────────────────────────────────────────────────

async fn sync_events(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "event.tar.xz",
        token,
        progress,
        base_url,
        parse_event_json,
        UpsertEvent,
    )
    .await
}

fn parse_event_json(json: &serde_json::Value, mbid: String) -> DbEvent {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let event_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let life_span = json.get("life-span");
    let begin_date = life_span
        .and_then(|ls| ls.get("begin"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let end_date = life_span
        .and_then(|ls| ls.get("end"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let time = json
        .get("time")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let cancelled = json
        .get("cancelled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    DbEvent {
        mbid,
        name,
        event_type,
        begin_date,
        end_date,
        time,
        cancelled,
        disambiguation,
        last_synced_at: Utc::now(),
    }
}

// ── Instrument sync ───────────────────────────────────────────────

async fn sync_instruments(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "instrument.tar.xz",
        token,
        progress,
        base_url,
        parse_instrument_json,
        UpsertInstrument,
    )
    .await
}

fn parse_instrument_json(json: &serde_json::Value, mbid: String) -> DbInstrument {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let instrument_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let description = json
        .get("description")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    DbInstrument {
        mbid,
        name,
        instrument_type,
        description,
        disambiguation,
        last_synced_at: Utc::now(),
    }
}

// ── Place sync ────────────────────────────────────────────────────

async fn sync_places(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    base_url: &str,
) -> Result<DatasetResult> {
    sync_generic(
        db,
        "place.tar.xz",
        token,
        progress,
        base_url,
        parse_place_json,
        UpsertPlace,
    )
    .await
}

fn parse_place_json(json: &serde_json::Value, mbid: String) -> DbPlace {
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let place_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let area = json
        .get("area")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let coordinates = json.get("coordinates").and_then(|c| {
        let lat = c.get("latitude").and_then(|v| v.as_str())?;
        let lon = c.get("longitude").and_then(|v| v.as_str())?;
        Some(format!("{},{}", lat, lon))
    });

    let disambiguation = json
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    DbPlace {
        mbid,
        name,
        place_type,
        area,
        coordinates,
        disambiguation,
        last_synced_at: Utc::now(),
    }
}
