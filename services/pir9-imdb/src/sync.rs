//! IMDB Dataset Sync
//!
//! Downloads and parses IMDB non-commercial datasets from https://datasets.imdbws.com/

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use std::collections::HashSet;
use std::sync::Arc;

use crate::db::{DbRepository, ResumeInfo};
use crate::models::*;

/// IMDB datasets base URL
const IMDB_BASE_URL: &str = "https://datasets.imdbws.com";

/// Progress log interval (rows processed between DB updates)
const PROGRESS_INTERVAL: i64 = 10_000;

/// Cancel check interval (check every N rows — is_cancelled() is just an atomic read)
const CANCEL_CHECK_INTERVAL: i64 = 10_000;

/// Batch size for DB upserts (rows per INSERT statement)
const BATCH_SIZE: usize = 1000;

/// Minimum hours between re-syncing the same dataset
const SKIP_IF_RECENT_HOURS: i64 = 24;

/// TV title types we care about
const TV_TITLE_TYPES: &[&str] = &["tvSeries", "tvMiniSeries"];

/// Movie title types we care about (includes TV movies and direct-to-video)
const MOVIE_TITLE_TYPES: &[&str] = &["movie", "tvMovie", "video"];

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
/// If PROCESS_ONLY_FLAG is set, uses cached file regardless of age (no download).
/// Returns the cache file path on success, or DatasetResult::Cancelled if the token fires.
async fn download_dataset(
    url: &str,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<std::result::Result<PathBuf, DatasetResult>> {
    // Ensure cache directory exists
    tokio::fs::create_dir_all(CACHE_DIR).await?;

    let cache_path = PathBuf::from(CACHE_DIR).join(dataset);

    // Process-only mode: use cached file regardless of age, fail if not cached
    // Check if another task is already downloading this dataset
    let lock_path = PathBuf::from(CACHE_DIR).join(format!("{}.downloading", dataset));
    let process_only = progress.read().await.process_only;
    if !process_only && lock_path.exists() {
        info!("{}: already being downloaded, skipping duplicate", dataset);
        return Ok(Ok(cache_path));
    }

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
        // Set progress to indicate we skipped downloading
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
        .user_agent("pir9-IMDB/0.1.0")
        .timeout(std::time::Duration::from_secs(3600))
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
        "Downloading {} → {:?} (size: {})",
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
            // Clean up temp file
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

    // Rename tmp → final atomically
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

/// All IMDB dataset filenames, in processing order
pub const ALL_DATASETS: &[&str] = &[
    "title.basics.tsv.gz",
    "title.episode.tsv.gz",
    "title.ratings.tsv.gz",
    "name.basics.tsv.gz",
    "title.principals.tsv.gz",
];

/// Check whether a dataset name (from the user request) matches a dataset filename.
/// Accepts both exact match ("title.basics.tsv.gz") and short name ("title.basics").
fn dataset_matches(filename: &str, requested: &str) -> bool {
    filename == requested || filename.starts_with(&format!("{}.", requested))
        || filename.trim_end_matches(".tsv.gz") == requested
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

    // Only make HEAD requests if cache is stale
    let remote_sizes = if need_refresh {
        info!("Remote size cache stale, fetching from upstream...");
        let client = reqwest::Client::builder()
            .user_agent("pir9-IMDB/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let mut sizes = std::collections::HashMap::new();
        for &dataset in ALL_DATASETS {
            let url = format!("{}/{}", IMDB_BASE_URL, dataset);
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
    let mut downloaded = Vec::new();

    for dataset in datasets {
        if token.is_cancelled() {
            break;
        }
        let url = format!("{}/{}", IMDB_BASE_URL, dataset);
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

        run_single_dataset(db, dataset, &token, &progress, &mut report).await;
    }

    // Reset flag
    {
        let mut p = progress.write().await;
        p.process_only = false;
    }

    Ok(report)
}

/// Run a full sync of all IMDB datasets (or a subset if requested)
pub async fn run_full_sync(
    db: &DbRepository,
    token: CancellationToken,
    progress: Arc<RwLock<DownloadProgress>>,
) -> Result<SyncReport> {
    run_full_sync_selective(db, &[], token, progress).await
}

/// Run a selective sync of the requested IMDB datasets (empty = all)
pub async fn run_full_sync_selective(
    db: &DbRepository,
    requested: &[String],
    token: CancellationToken,
    progress: Arc<RwLock<DownloadProgress>>,
) -> Result<SyncReport> {
    let datasets = resolve_datasets(requested);
    let mut report = SyncReport::default();

    for &dataset in &datasets {
        if token.is_cancelled() {
            return Ok(report);
        }
        run_single_dataset(db, dataset, &token, &progress, &mut report).await;
    }

    Ok(report)
}

/// Run sync for a single named dataset, updating the report
async fn run_single_dataset(
    db: &DbRepository,
    dataset: &str,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
    report: &mut SyncReport,
) {
    info!("Syncing {}...", dataset);

    let result = match dataset {
        "title.basics.tsv.gz" => sync_title_basics(db, token, progress).await,
        "title.episode.tsv.gz" => sync_title_episodes(db, token, progress).await,
        "title.ratings.tsv.gz" => sync_title_ratings(db, token, progress).await,
        "name.basics.tsv.gz" => sync_name_basics(db, token, progress).await,
        "title.principals.tsv.gz" => sync_title_principals(db, token, progress).await,
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
                "title.basics.tsv.gz" => report.title_basics = Some(stats),
                "title.episode.tsv.gz" => report.title_episodes = Some(stats),
                "title.ratings.tsv.gz" => report.title_ratings = Some(stats),
                "name.basics.tsv.gz" => report.name_basics = Some(stats),
                "title.principals.tsv.gz" => report.title_principals = Some(stats),
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
/// Returns (sync_id, ResumeInfo) where ResumeInfo contains the checkpoint and old counters.
async fn get_or_resume_sync(db: &DbRepository, dataset: &str) -> Result<(i64, ResumeInfo)> {
    // Check for a resumable sync first
    if let Some(resume) = db.get_resumable_sync(dataset).await? {
        info!(
            "Resuming {} from sync_id={}, last_processed_id={}, prior progress: {} processed",
            dataset, resume.sync_id, resume.last_processed_id, resume.rows_processed
        );
        db.resume_sync(resume.sync_id).await?;
        let sync_id = resume.sync_id;
        return Ok((sync_id, resume));
    }

    // No resumable sync, create a new one
    let sync_id = db.start_sync(dataset).await?;
    Ok((sync_id, ResumeInfo {
        sync_id,
        last_processed_id: 0,
        rows_processed: 0,
        rows_inserted: 0,
        rows_updated: 0,
    }))
}

/// Sync title.basics.tsv.gz (TV series)
async fn sync_title_basics(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.basics.tsv.gz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_basics_inner(db, &url, sync_id, &resume, token, progress).await;

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

async fn sync_title_basics_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.basics.tsv.gz";
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let file = std::fs::File::open(&cache_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    // Start counters from previous values when resuming
    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut rows_skipped: i64 = 0;
    let mut last_id: i64 = 0;

    // Batch buffers
    let mut series_batch: Vec<DbSeries> = Vec::with_capacity(BATCH_SIZE);
    let mut movie_batch: Vec<DbMovie> = Vec::with_capacity(BATCH_SIZE);

    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue; // Skip header
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 9 {
            continue;
        }

        let title_type = fields[1];
        let is_tv = TV_TITLE_TYPES.contains(&title_type);
        let is_movie = MOVIE_TITLE_TYPES.contains(&title_type);

        // Only process TV series and movie types
        if !is_tv && !is_movie {
            continue;
        }

        let imdb_id = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        // Skip rows we've already processed (resume support)
        if imdb_id <= resume_from {
            rows_skipped += 1;
            continue;
        }

        if is_tv {
            series_batch.push(DbSeries {
                imdb_id,
                title: fields[2].to_string(),
                original_title: if fields[3] != "\\N" && fields[3] != fields[2] {
                    Some(fields[3].to_string())
                } else {
                    None
                },
                is_adult: fields[4] == "1",
                start_year: parse_int(fields[5]),
                end_year: parse_int(fields[6]),
                runtime_minutes: parse_int(fields[7]),
                genres: if fields[8] != "\\N" {
                    Some(fields[8].to_string())
                } else {
                    None
                },
                title_type: title_type.to_string(),
                rating: None,
                votes: None,
                last_synced_at: Utc::now(),
            });

            if series_batch.len() >= BATCH_SIZE {
                rows_inserted += flush_series(db, &mut series_batch).await? as i64;
            }
        } else {
            movie_batch.push(DbMovie {
                imdb_id,
                title: fields[2].to_string(),
                original_title: if fields[3] != "\\N" && fields[3] != fields[2] {
                    Some(fields[3].to_string())
                } else {
                    None
                },
                is_adult: fields[4] == "1",
                year: parse_int(fields[5]),
                runtime_minutes: parse_int(fields[7]),
                genres: if fields[8] != "\\N" {
                    Some(fields[8].to_string())
                } else {
                    None
                },
                rating: None,
                votes: None,
                last_synced_at: Utc::now(),
                tmdb_id: None,
                poster_url: None,
                fanart_url: None,
                tmdb_fetched_at: None,
            });

            if movie_batch.len() >= BATCH_SIZE {
                rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;
            }
        }

        rows_processed += 1;
        last_id = imdb_id;

        if rows_processed % PROGRESS_INTERVAL == 0 {
            // Flush before checkpoint so last_processed_id is never ahead of DB state
            rows_inserted += flush_series(db, &mut series_batch).await? as i64;
            rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;

            if rows_skipped > 0 {
                info!(
                    "title.basics progress: {} rows (skipped {} resumed)",
                    rows_processed, rows_skipped
                );
                rows_skipped = 0; // Only log once
            } else {
                info!("title.basics progress: {} rows", rows_processed);
            }
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
        }

        // Cooperative cancellation check
        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_inserted += flush_series(db, &mut series_batch).await? as i64;
            rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;
            info!(
                "title.basics cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_inserted += flush_series(db, &mut series_batch).await? as i64;
    rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

/// Sync title.episode.tsv.gz (episodes)
async fn sync_title_episodes(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.episode.tsv.gz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_episodes_inner(db, &url, sync_id, &resume, token, progress).await;

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

async fn sync_title_episodes_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.episode.tsv.gz";
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    // First, get all our series IDs for filtering
    let series_ids: HashSet<i64> =
        sqlx::query_scalar("SELECT imdb_id FROM imdb_series")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    info!("Filtering episodes for {} series", series_ids.len());

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let file = std::fs::File::open(&cache_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    // Start counters from previous values when resuming
    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id: i64 = 0;

    let mut episode_batch: Vec<DbEpisode> = Vec::with_capacity(BATCH_SIZE);

    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            continue;
        }

        let episode_id = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        let parent_id = match parse_imdb_id(fields[1]) {
            Some(id) => id,
            None => continue,
        };

        // Only process episodes for series we have
        if !series_ids.contains(&parent_id) {
            continue;
        }

        // Skip rows we've already processed (resume support)
        if episode_id <= resume_from {
            continue;
        }

        episode_batch.push(DbEpisode {
            imdb_id: episode_id,
            parent_imdb_id: parent_id,
            season_number: parse_int(fields[2]),
            episode_number: parse_int(fields[3]),
            title: None,
            runtime_minutes: None,
            rating: None,
            votes: None,
            air_date: None,
            last_synced_at: Utc::now(),
        });

        if episode_batch.len() >= BATCH_SIZE {
            rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;
        }

        rows_processed += 1;
        last_id = episode_id;

        if rows_processed % PROGRESS_INTERVAL == 0 {
            rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;
            info!("title.episode progress: {} rows", rows_processed);
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                rows_inserted,
                0,
                last_id,
            )
            .await?;
        }

        // Cooperative cancellation check
        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;
            info!(
                "title.episode cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                rows_inserted,
                0,
                last_id,
            )
            .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

/// Sync title.ratings.tsv.gz
async fn sync_title_ratings(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.ratings.tsv.gz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_ratings_inner(db, &url, sync_id, &resume, token, progress).await;

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

async fn sync_title_ratings_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.ratings.tsv.gz";
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let file = std::fs::File::open(&cache_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    // Start counters from previous values when resuming
    let mut rows_processed = resume.rows_processed;
    let mut rows_updated = resume.rows_updated;
    let mut last_id: i64 = 0;

    // Batch buffers: (imdb_id, rating, votes)
    let mut rating_ids: Vec<i64> = Vec::with_capacity(BATCH_SIZE);
    let mut rating_vals: Vec<f64> = Vec::with_capacity(BATCH_SIZE);
    let mut rating_votes: Vec<i64> = Vec::with_capacity(BATCH_SIZE);

    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }

        let imdb_id = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        // Skip rows we've already processed (resume support)
        if imdb_id <= resume_from {
            continue;
        }

        let rating: f64 = match fields[1].parse() {
            Ok(r) => r,
            Err(_) => continue,
        };

        let votes: i64 = match fields[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        rating_ids.push(imdb_id);
        rating_vals.push(rating);
        rating_votes.push(votes);

        if rating_ids.len() >= BATCH_SIZE {
            rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;
        }

        rows_processed += 1;
        last_id = imdb_id;

        if rows_processed % (PROGRESS_INTERVAL * 10) == 0 {
            rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;
            info!("title.ratings progress: {} rows", rows_processed);
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                0,
                rows_updated,
                last_id,
            )
            .await?;
        }

        // Cooperative cancellation check
        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;
            info!(
                "title.ratings cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                0,
                rows_updated,
                last_id,
            )
            .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;

    db.update_sync_progress_with_resume(sync_id, rows_processed, 0, rows_updated, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted: 0,
        rows_updated,
        duration_seconds: 0,
    }))
}

// ── name.basics sync ────────────────────────────────────────────────

/// Sync name.basics.tsv.gz (people, pre-filtered by known titles)
async fn sync_name_basics(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "name.basics.tsv.gz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_name_basics_inner(db, &url, sync_id, &resume, token, progress).await;

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

async fn sync_name_basics_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "name.basics.tsv.gz";
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    // Load known title IDs (series + movies) for pre-filtering
    let series_ids: HashSet<i64> =
        sqlx::query_scalar("SELECT imdb_id FROM imdb_series")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    let movie_ids: HashSet<i64> =
        sqlx::query_scalar("SELECT imdb_id FROM imdb_movies")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    let title_ids: HashSet<i64> = series_ids.union(&movie_ids).copied().collect();
    info!(
        "Pre-filter: {} known titles ({} series + {} movies)",
        title_ids.len(),
        series_ids.len(),
        movie_ids.len()
    );

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let file = std::fs::File::open(&cache_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id: i64 = 0;

    let mut people_batch: Vec<DbPerson> = Vec::with_capacity(BATCH_SIZE);

    // name.basics.tsv format:
    // nconst \t primaryName \t birthYear \t deathYear \t primaryProfession \t knownForTitles
    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue; // Skip header
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 6 {
            continue;
        }

        let nconst = match parse_nconst(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        // Skip rows we've already processed (resume support)
        if nconst <= resume_from {
            continue;
        }

        // Pre-filter: skip person if none of their knownForTitles are in our set
        let known_for = fields[5];
        if known_for == "\\N" || known_for.is_empty() {
            continue;
        }

        let has_known_title = known_for.split(',').any(|tt| {
            if tt.len() > 2 && tt.starts_with("tt") {
                tt[2..].parse::<i64>().ok().map_or(false, |id| title_ids.contains(&id))
            } else {
                false
            }
        });

        if !has_known_title {
            continue;
        }

        people_batch.push(DbPerson {
            nconst,
            primary_name: fields[1].to_string(),
            birth_year: parse_smallint(fields[2]),
            death_year: parse_smallint(fields[3]),
            primary_profession: if fields[4] != "\\N" {
                Some(fields[4].to_string())
            } else {
                None
            },
            known_for_titles: if known_for != "\\N" {
                // Store as comma-separated numeric IDs
                let numeric: String = known_for
                    .split(',')
                    .filter_map(|tt| {
                        if tt.len() > 2 && tt.starts_with("tt") {
                            tt[2..].parse::<i64>().ok().map(|id| id.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                if numeric.is_empty() { None } else { Some(numeric) }
            } else {
                None
            },
        });

        if people_batch.len() >= BATCH_SIZE {
            rows_inserted += flush_people(db, &mut people_batch).await? as i64;
        }

        rows_processed += 1;
        last_id = nconst;

        if rows_processed % PROGRESS_INTERVAL == 0 {
            rows_inserted += flush_people(db, &mut people_batch).await? as i64;
            info!("name.basics progress: {} rows", rows_processed);
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
        }

        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_inserted += flush_people(db, &mut people_batch).await? as i64;
            info!(
                "name.basics cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_inserted += flush_people(db, &mut people_batch).await? as i64;

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

// ── title.principals sync ───────────────────────────────────────────

/// Sync title.principals.tsv.gz (credits, dual pre-filtered by titles + people)
async fn sync_title_principals(
    db: &DbRepository,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.principals.tsv.gz";

    if should_skip_dataset(db, dataset, progress).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_principals_inner(db, &url, sync_id, &resume, token, progress).await;

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

async fn sync_title_principals_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
    progress: &Arc<RwLock<DownloadProgress>>,
) -> Result<DatasetResult> {
    let dataset = "title.principals.tsv.gz";
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    // Dual pre-filter: load known title IDs and person IDs
    let series_ids: HashSet<i64> =
        sqlx::query_scalar("SELECT imdb_id FROM imdb_series")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    let movie_ids: HashSet<i64> =
        sqlx::query_scalar("SELECT imdb_id FROM imdb_movies")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    let title_ids: HashSet<i64> = series_ids.union(&movie_ids).copied().collect();

    let person_ids: HashSet<i64> =
        sqlx::query_scalar("SELECT nconst FROM imdb_people")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    info!(
        "Pre-filter: {} known titles, {} known people",
        title_ids.len(),
        person_ids.len()
    );

    let cache_path = match download_dataset(url, dataset, token, progress).await? {
        Ok(p) => p,
        Err(cancelled) => return Ok(cancelled),
    };

    let file = std::fs::File::open(&cache_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id: i64 = 0;

    let mut credit_batch: Vec<DbCredit> = Vec::with_capacity(BATCH_SIZE);

    // title.principals.tsv format:
    // tconst \t ordering \t nconst \t category \t job \t characters
    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue; // Skip header
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 6 {
            continue;
        }

        let tconst = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        // Dual pre-filter: skip if title not in our set
        if !title_ids.contains(&tconst) {
            continue;
        }

        let nconst = match parse_nconst(fields[2]) {
            Some(id) => id,
            None => continue,
        };

        // Dual pre-filter: skip if person not in our set
        if !person_ids.contains(&nconst) {
            continue;
        }

        let ordering: i16 = match fields[1].parse() {
            Ok(o) => o,
            Err(_) => continue,
        };

        // Skip rows we've already processed (resume support)
        // title.principals doesn't have monotonic IDs per se, but tconst is close enough
        // We use a composite key: tconst * 100 + ordering as a rough resume marker
        let resume_key = tconst * 100 + ordering as i64;
        if resume_key <= resume_from {
            continue;
        }

        credit_batch.push(DbCredit {
            tconst,
            nconst,
            ordering,
            category: fields[3].to_string(),
            job: if fields[4] != "\\N" {
                Some(fields[4].to_string())
            } else {
                None
            },
            characters: if fields[5] != "\\N" {
                Some(fields[5].to_string())
            } else {
                None
            },
        });

        if credit_batch.len() >= BATCH_SIZE {
            rows_inserted += flush_credits(db, &mut credit_batch).await? as i64;
        }

        rows_processed += 1;
        last_id = resume_key;

        if rows_processed % PROGRESS_INTERVAL == 0 {
            rows_inserted += flush_credits(db, &mut credit_batch).await? as i64;
            info!("title.principals progress: {} rows", rows_processed);
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
        }

        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_inserted += flush_credits(db, &mut credit_batch).await? as i64;
            info!(
                "title.principals cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_inserted += flush_credits(db, &mut credit_batch).await? as i64;

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

// ── Batch flush helpers ──────────────────────────────────────────────

async fn flush_series(db: &DbRepository, batch: &mut Vec<DbSeries>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_series_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_movies(db: &DbRepository, batch: &mut Vec<DbMovie>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_movie_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_episodes(db: &DbRepository, batch: &mut Vec<DbEpisode>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_episode_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_ratings(
    db: &DbRepository,
    ids: &mut Vec<i64>,
    ratings: &mut Vec<f64>,
    votes: &mut Vec<i64>,
) -> Result<i64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let series_count = db.update_series_ratings_batch(ids, ratings, votes).await?;
    let movie_count = db.update_movie_ratings_batch(ids, ratings, votes).await?;
    ids.clear();
    ratings.clear();
    votes.clear();
    Ok((series_count + movie_count) as i64)
}

async fn flush_people(db: &DbRepository, batch: &mut Vec<DbPerson>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_people_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_credits(db: &DbRepository, batch: &mut Vec<DbCredit>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_credits_batch(batch).await?;
    batch.clear();
    Ok(count)
}

/// Parse an integer from a string, handling IMDB null values
fn parse_int<T: std::str::FromStr>(s: &str) -> Option<T> {
    if s == "\\N" || s.is_empty() {
        None
    } else {
        s.parse().ok()
    }
}

/// Parse a smallint (i16) from a string, handling IMDB null values
fn parse_smallint(s: &str) -> Option<i16> {
    if s == "\\N" || s.is_empty() {
        None
    } else {
        s.parse().ok()
    }
}

/// Parse IMDB ID string to numeric
fn parse_imdb_id(s: &str) -> Option<i64> {
    if s.len() > 2 && s.starts_with("tt") {
        s[2..].parse().ok()
    } else {
        None
    }
}

/// Parse IMDB person ID (nconst) to numeric
fn parse_nconst(s: &str) -> Option<i64> {
    if s.len() > 2 && s.starts_with("nm") {
        s[2..].parse().ok()
    } else {
        None
    }
}
